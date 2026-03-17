/// Constraint-based greedy scheduler with diversity-spread ordering.
///
/// Hard constraints:
///   1. No room double-booked at same (day, slot)
///   2. No lecturer double-booked
///   3. No batch scheduled twice at same time
///   4. Room type matches course requirement (lab ↔ lab, lecture/tutorial ↔ lecture)
///   5. Room capacity ≥ batch size
///   6. Lecturer available on the given day
///   7. Lecturer max_hours_per_day and max_hours_per_week respected
///   8. Lecturer blackout slots/days respected
///   9. Lecturer max_consecutive_hours respected
///
/// Soft constraints (affect placement priority):
///   - Preferred time-of-day per day (morning/afternoon)
///
/// Diversity heuristics:
///   - Candidates sorted by: (batch_day_count, slot_penalty+preferred_penalty, day_index, slot)
///   - Labs prefer afternoon slots (4–7); tutorials prefer mornings
///   - Biweekly courses are placed with week_parity=1 (odd teaching weeks only)
///   - Most-hours-first need ordering prevents starvation

use std::collections::{HashMap, HashSet};
use crate::models::*;

pub struct SchedulerInput {
    pub courses: Vec<Course>,
    pub lecturers: Vec<Lecturer>,
    pub rooms: Vec<Room>,
    pub batches: Vec<Batch>,
    /// Days the organization schedules on, in desired column order.
    /// Defaults to Mon–Fri if empty.
    pub working_days: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PlacedEntry {
    pub course_id: i64,
    pub lecturer_id: i64,
    pub room_id: i64,
    pub batch_id: i64,
    pub day: String,
    pub time_slot: i64,
    pub class_type: String,
    pub week_parity: i64,  // 0=every week, 1=odd weeks, 2=even weeks
}

pub struct ScheduleResult {
    pub entries: Vec<PlacedEntry>,
    pub unscheduled: Vec<UnscheduledItem>,
}

// Slot preference score — lower = more preferred for a given class type
fn slot_penalty(class_type: &str, slot: i64) -> i64 {
    match class_type {
        "lab" => {
            // Labs prefer afternoon (slots 4-7)
            if slot >= 4 { 0 } else { 3 }
        }
        "tutorial" => {
            // Tutorials prefer morning (slots 0-3)
            if slot < 4 { 0 } else { 2 }
        }
        _ => {
            // Lectures are fine any time but avoid very early/late slots slightly
            if slot == 0 || slot == 7 { 1 } else { 0 }
        }
    }
}

/// Penalty for violating lecturer's preferred time-of-day for a given day.
/// preferred_slots_json: {"Mon":"morning","Tue":"afternoon",...}
fn preferred_penalty(preferred_json: &Option<String>, day: &str, slot: i64) -> i64 {
    let json = match preferred_json {
        Some(s) if !s.is_empty() => s,
        _ => return 0,
    };
    let map: HashMap<String, String> = serde_json::from_str(json).unwrap_or_default();
    match map.get(day).map(|s| s.as_str()) {
        Some("morning") if slot >= 4 => 2,
        Some("afternoon") if slot < 4 => 2,
        _ => 0,
    }
}

/// Returns true if the given (day, slot) is in the lecturer's blackout list.
/// blackout_json: [{"day":"Mon","slot":null},{"day":"Fri","slot":2},...]
/// slot: null means the entire day is blacked out for this lecturer.
fn is_blacked_out(blackout_json: &Option<String>, day: &str, slot: i64) -> bool {
    let json = match blackout_json {
        Some(s) if !s.is_empty() => s,
        _ => return false,
    };
    let arr: Vec<serde_json::Value> = serde_json::from_str(json).unwrap_or_default();
    for item in &arr {
        let item_day = item["day"].as_str().unwrap_or("");
        if item_day != day { continue; }
        // null slot = entire day blocked
        if item["slot"].is_null() { return true; }
        if item["slot"].as_i64() == Some(slot) { return true; }
    }
    false
}

/// Returns true if adding new_slot to occupied_slots would create a consecutive
/// run exceeding max_consecutive. Slots 3→4 are NOT consecutive (lunch gap).
fn would_exceed_consecutive(occupied: &[i64], new_slot: i64, max_consecutive: i64) -> bool {
    if max_consecutive <= 0 { return false; }
    let mut all: Vec<i64> = occupied.to_vec();
    all.push(new_slot);
    all.sort_unstable();
    let mut run = 1i64;
    for i in 1..all.len() {
        let a = all[i - 1];
        let b = all[i];
        // Slots 3 (11:00) and 4 (13:00) have a lunch gap — not consecutive
        if b == a + 1 && !(a == 3 && b == 4) {
            run += 1;
            if run > max_consecutive { return true; }
        } else {
            run = 1;
        }
    }
    false
}

pub fn generate(input: &SchedulerInput) -> ScheduleResult {
    // Resolve working days: use org config or fall back to Mon–Fri
    let default_days = vec![
        "Mon".to_string(), "Tue".to_string(), "Wed".to_string(),
        "Thu".to_string(), "Fri".to_string(),
    ];
    let org_days_owned: &Vec<String> = if input.working_days.is_empty() {
        &default_days
    } else {
        &input.working_days
    };
    let working_days: Vec<&str> = org_days_owned.iter().map(|s| s.as_str()).collect();

    let courses: HashMap<i64, &Course> = input.courses.iter().map(|c| (c.id, c)).collect();
    let lecturers: HashMap<i64, &Lecturer> = input.lecturers.iter().map(|l| (l.id, l)).collect();
    let rooms: Vec<&Room> = input.rooms.iter().collect();

    type DaySlot = (String, i64);

    let mut room_busy: HashMap<i64, HashSet<DaySlot>> = HashMap::new();
    let mut lecturer_busy: HashMap<i64, HashSet<DaySlot>> = HashMap::new();
    let mut batch_busy: HashMap<i64, HashSet<DaySlot>> = HashMap::new();
    let mut lecturer_day_load: HashMap<(i64, String), i64> = HashMap::new();
    let mut lecturer_week_load: HashMap<i64, i64> = HashMap::new();
    // For diversity: track how many sessions each (batch, day) already has
    let mut batch_day_count: HashMap<(i64, String), i64> = HashMap::new();
    // For consecutive-hours check: track which slots are occupied per (lecturer, day)
    let mut lecturer_day_slots: HashMap<(i64, String), Vec<i64>> = HashMap::new();

    // Build needs: (batch_id, course_id, hours_to_place)
    // For biweekly: place ceil(hours/2) sessions (they show every-other-week in calendar)
    let mut needs: Vec<(i64, i64, i64, bool)> = Vec::new(); // (batch, course, hours, is_biweekly)
    for batch in &input.batches {
        for &cid in &batch.course_ids {
            if let Some(course) = courses.get(&cid) {
                let biweekly = course.frequency == "biweekly";
                let hours = if biweekly {
                    (course.hours_per_week + 1) / 2
                } else {
                    course.hours_per_week
                };
                needs.push((batch.id, cid, hours, biweekly));
            }
        }
    }

    // Sort: most sessions first → harder to place, schedule early
    needs.sort_by(|a, b| b.2.cmp(&a.2));

    let mut entries: Vec<PlacedEntry> = Vec::new();
    let mut unscheduled: Vec<UnscheduledItem> = Vec::new();

    'need: for (batch_id, course_id, hours_needed, is_biweekly) in &needs {
        let batch = match input.batches.iter().find(|b| b.id == *batch_id) {
            Some(b) => b,
            None => continue,
        };
        let course = match courses.get(course_id) {
            Some(c) => c,
            None => continue,
        };
        let lecturer_id = match course.lecturer_id {
            Some(lid) => lid,
            None => {
                unscheduled.push(UnscheduledItem {
                    batch_name: batch.name.clone(),
                    course_code: course.code.clone(),
                    course_name: course.name.clone(),
                    hours_needed: *hours_needed,
                    reason: "No lecturer assigned to course".into(),
                });
                continue;
            }
        };
        let lecturer = match lecturers.get(&lecturer_id) {
            Some(l) => l,
            None => {
                unscheduled.push(UnscheduledItem {
                    batch_name: batch.name.clone(),
                    course_code: course.code.clone(),
                    course_name: course.name.clone(),
                    hours_needed: *hours_needed,
                    reason: "Assigned lecturer not found".into(),
                });
                continue;
            }
        };

        let lec_avail_days: HashSet<&str> =
            lecturer.available_days.split(',').map(|s| s.trim()).collect();

        let class_type = &course.class_type;

        let mut placed = 0i64;

        // Build candidate (day, slot) list sorted by diversity score
        let mut candidates: Vec<(&str, i64)> = working_days
            .iter()
            .flat_map(|&d| TIME_SLOTS.iter().map(move |&s| (d, s)))
            .collect();

        // Sort candidates: (batch_day_count, slot_penalty + preferred_penalty, day_idx, slot)
        candidates.sort_by_key(|(day, slot)| {
            let bdc = *batch_day_count
                .get(&(*batch_id, day.to_string()))
                .unwrap_or(&0);
            let sp = slot_penalty(class_type, *slot);
            let pp = preferred_penalty(&lecturer.preferred_slots_json, day, *slot);
            let di = working_days.iter().position(|&d| d == *day).unwrap_or(0) as i64;
            (bdc, sp + pp, di, *slot)
        });

        for (day, slot) in &candidates {
            if placed >= *hours_needed {
                break;
            }

            let ds = (day.to_string(), *slot);

            // 1. Lecturer available on this day?
            if !lec_avail_days.contains(*day) { continue; }

            // 2. Blackout check (hard constraint from soft preferences)
            if is_blacked_out(&lecturer.blackout_json, day, *slot) { continue; }

            // 3. Lecturer daily load
            let day_load = lecturer_day_load
                .get(&(lecturer_id, day.to_string()))
                .copied().unwrap_or(0);
            if day_load >= lecturer.max_hours_per_day { continue; }

            // 4. Lecturer weekly load
            let week_load = lecturer_week_load.get(&lecturer_id).copied().unwrap_or(0);
            if week_load >= lecturer.max_hours_per_week {
                unscheduled.push(UnscheduledItem {
                    batch_name: batch.name.clone(),
                    course_code: course.code.clone(),
                    course_name: course.name.clone(),
                    hours_needed: *hours_needed - placed,
                    reason: format!(
                        "Lecturer '{}' reached weekly max ({} h)",
                        lecturer.name, lecturer.max_hours_per_week
                    ),
                });
                continue 'need;
            }

            // 5. Consecutive hours check
            let occupied = lecturer_day_slots
                .get(&(lecturer_id, day.to_string()))
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            if would_exceed_consecutive(occupied, *slot, lecturer.max_consecutive_hours) {
                continue;
            }

            // 6. Lecturer not double-booked
            if lecturer_busy.get(&lecturer_id).map_or(false, |s| s.contains(&ds)) { continue; }

            // 7. Batch not double-booked
            if batch_busy.get(batch_id).map_or(false, |s| s.contains(&ds)) { continue; }

            // 8. Find a suitable room
            let room = rooms.iter().find(|r| {
                r.room_type == course.room_type
                    && r.capacity >= batch.size
                    && r.available_days.split(',').any(|d| d.trim() == *day)
                    && !room_busy.get(&r.id).map_or(false, |s| s.contains(&ds))
            });

            let room = match room {
                Some(r) => r,
                None => continue,
            };

            // Assign
            room_busy.entry(room.id).or_default().insert(ds.clone());
            lecturer_busy.entry(lecturer_id).or_default().insert(ds.clone());
            batch_busy.entry(*batch_id).or_default().insert(ds.clone());
            *lecturer_day_load.entry((lecturer_id, day.to_string())).or_insert(0) += 1;
            *lecturer_week_load.entry(lecturer_id).or_insert(0) += 1;
            *batch_day_count.entry((*batch_id, day.to_string())).or_insert(0) += 1;
            lecturer_day_slots.entry((lecturer_id, day.to_string())).or_default().push(*slot);

            entries.push(PlacedEntry {
                course_id: *course_id,
                lecturer_id,
                room_id: room.id,
                batch_id: *batch_id,
                day: day.to_string(),
                time_slot: *slot,
                class_type: class_type.clone(),
                week_parity: if *is_biweekly { 1 } else { 0 },
            });
            placed += 1;
        }

        if placed < *hours_needed {
            unscheduled.push(UnscheduledItem {
                batch_name: batch.name.clone(),
                course_code: course.code.clone(),
                course_name: course.name.clone(),
                hours_needed: *hours_needed - placed,
                reason: format!(
                    "Could only place {}/{} sessions — no valid slot/room for remaining",
                    placed, hours_needed
                ),
            });
        }
    }

    ScheduleResult { entries, unscheduled }
}

// ══════════════════════════════════════════════════════════════════════════════
// ALGORITHM 2 — CSP Greedy with Dynamic MCV Ordering + Backjump Recovery
// ══════════════════════════════════════════════════════════════════════════════
//
// Key differences from Algorithm 1 (static greedy):
//   - Dynamic MCV: before each placement, re-rank remaining needs by domain
//     size (number of still-valid (day, slot, room) triples).  The most
//     constrained need is placed first, reducing cascading failures.
//   - LCV slot selection: among valid candidates for a given need, prefer the
//     slot that eliminates the fewest options for *other* unscheduled needs.
//   - Backjump recovery: when a need has zero valid placements, find the most
//     recently placed entry that "conflicts" with it and try to relocate that
//     entry to an alternative slot.  Up to MAX_BACKJUMPS per schedule run.
//
// This trades raw throughput for higher placement rate on large/dense datasets.

/// Maximum number of backjump recovery attempts per schedule run.
const MAX_BACKJUMPS: usize = 300;

/// Count how many valid (day, slot, room) triples remain for a need, given
/// the current booking state.  Used for MCV domain-size estimation.
fn domain_size(
    batch_id: i64,
    course: &Course,
    lecturer: &Lecturer,
    working_days: &[&str],
    room_busy: &HashMap<i64, HashSet<(String, i64)>>,
    lecturer_busy: &HashMap<i64, HashSet<(String, i64)>>,
    batch_busy: &HashMap<i64, HashSet<(String, i64)>>,
    lecturer_day_load: &HashMap<(i64, String), i64>,
    lecturer_week_load: &HashMap<i64, i64>,
    lecturer_day_slots: &HashMap<(i64, String), Vec<i64>>,
    rooms: &[&Room],
) -> usize {
    let lec_avail: HashSet<&str> = lecturer.available_days.split(',').map(|s| s.trim()).collect();
    let lec_id = lecturer.id;
    let week_load = lecturer_week_load.get(&lec_id).copied().unwrap_or(0);
    if week_load >= lecturer.max_hours_per_week {
        return 0;
    }
    let mut count = 0usize;
    for &day in working_days {
        if !lec_avail.contains(day) { continue; }
        if is_blacked_out(&lecturer.blackout_json, day, 0)
            && is_blacked_out(&lecturer.blackout_json, day, 7)
        {
            continue; // entire day blacked out (approximate)
        }
        let day_load = lecturer_day_load.get(&(lec_id, day.to_string())).copied().unwrap_or(0);
        if day_load >= lecturer.max_hours_per_day { continue; }
        for &slot in TIME_SLOTS {
            if is_blacked_out(&lecturer.blackout_json, day, slot) { continue; }
            let ds = (day.to_string(), slot);
            if lecturer_busy.get(&lec_id).map_or(false, |s| s.contains(&ds)) { continue; }
            if batch_busy.get(&batch_id).map_or(false, |s| s.contains(&ds)) { continue; }
            let occupied = lecturer_day_slots.get(&(lec_id, day.to_string()))
                .map(|v| v.as_slice()).unwrap_or(&[]);
            if would_exceed_consecutive(occupied, slot, lecturer.max_consecutive_hours) { continue; }
            // Count rooms that fit
            let room_count = rooms.iter().filter(|r| {
                r.room_type == course.room_type
                    && r.capacity >= course.hours_per_week // proxy for batch size, overridden below
                    && r.available_days.split(',').any(|d| d.trim() == day)
                    && !room_busy.get(&r.id).map_or(false, |s| s.contains(&ds))
            }).count();
            if room_count > 0 { count += 1; }
        }
    }
    count
}

/// LCV score: how many valid slots does this candidate eliminate for OTHER
/// unscheduled needs?  Lower = better (least constraining).
fn lcv_cost(
    day: &str,
    slot: i64,
    room_id: i64,
    lec_id: i64,
    batch_id: i64,
    remaining_needs: &[(i64, i64, i64, bool)],
    courses: &HashMap<i64, &Course>,
    lecturers: &HashMap<i64, &Lecturer>,
    _working_days: &[&str],
    room_busy: &HashMap<i64, HashSet<(String, i64)>>,
    lecturer_busy: &HashMap<i64, HashSet<(String, i64)>>,
    batch_busy: &HashMap<i64, HashSet<(String, i64)>>,
) -> i64 {
    let ds = (day.to_string(), slot);
    let mut cost = 0i64;
    for &(b_id, c_id, _, _) in remaining_needs {
        let course = match courses.get(&c_id) { Some(c) => c, None => continue };
        let lec = match course.lecturer_id.and_then(|l| lecturers.get(&l)) {
            Some(l) => l, None => continue,
        };
        // If this placement would block the other need's lecturer or batch at (day, slot)
        if lec.id == lec_id
            && !lecturer_busy.get(&lec_id).map_or(false, |s| s.contains(&ds))
        {
            cost += 1;
        }
        if b_id == batch_id
            && !batch_busy.get(&batch_id).map_or(false, |s| s.contains(&ds))
        {
            cost += 1;
        }
        // Room contention
        if course.room_type == courses.get(&c_id).map(|c| c.room_type.as_str()).unwrap_or("")
            && !room_busy.get(&room_id).map_or(false, |s| s.contains(&ds))
        {
            cost += 1;
        }
    }
    cost
}

pub fn generate_csp(input: &SchedulerInput) -> ScheduleResult {
    // Resolve working days
    let default_days = vec![
        "Mon".to_string(), "Tue".to_string(), "Wed".to_string(),
        "Thu".to_string(), "Fri".to_string(),
    ];
    let org_days_owned: &Vec<String> = if input.working_days.is_empty() {
        &default_days
    } else {
        &input.working_days
    };
    let working_days: Vec<&str> = org_days_owned.iter().map(|s| s.as_str()).collect();

    let courses: HashMap<i64, &Course> = input.courses.iter().map(|c| (c.id, c)).collect();
    let lecturers: HashMap<i64, &Lecturer> = input.lecturers.iter().map(|l| (l.id, l)).collect();
    let rooms: Vec<&Room> = input.rooms.iter().collect();

    type DaySlot = (String, i64);

    let mut room_busy: HashMap<i64, HashSet<DaySlot>> = HashMap::new();
    let mut lecturer_busy: HashMap<i64, HashSet<DaySlot>> = HashMap::new();
    let mut batch_busy: HashMap<i64, HashSet<DaySlot>> = HashMap::new();
    let mut lecturer_day_load: HashMap<(i64, String), i64> = HashMap::new();
    let mut lecturer_week_load: HashMap<i64, i64> = HashMap::new();
    let mut batch_day_count: HashMap<(i64, String), i64> = HashMap::new();
    let mut lecturer_day_slots: HashMap<(i64, String), Vec<i64>> = HashMap::new();

    // Build needs: (batch_id, course_id, hours_to_place, is_biweekly)
    let mut needs: Vec<(i64, i64, i64, bool)> = Vec::new();
    for batch in &input.batches {
        for &cid in &batch.course_ids {
            if let Some(course) = courses.get(&cid) {
                let biweekly = course.frequency == "biweekly";
                let hours = if biweekly { (course.hours_per_week + 1) / 2 } else { course.hours_per_week };
                needs.push((batch.id, cid, hours, biweekly));
            }
        }
    }

    // Placed entries as a stack so we can backjump
    let mut entries: Vec<PlacedEntry> = Vec::new();
    let mut unscheduled: Vec<UnscheduledItem> = Vec::new();

    // Track which needs are still pending (index into `needs`)
    let mut pending: Vec<usize> = (0..needs.len()).collect();
    let mut backjumps_used = 0usize;

    while !pending.is_empty() {
        // ── MCV: pick the need with the smallest domain size ─────────────────
        let chosen_pos = {
            let mut best_pos = 0usize;
            let mut best_domain = usize::MAX;
            for (pos, &idx) in pending.iter().enumerate() {
                let (batch_id, course_id, _, _) = needs[idx];
                let course = match courses.get(&course_id) { Some(c) => c, None => continue };
                let lec_id = match course.lecturer_id { Some(l) => l, None => { best_pos = pos; break; } };
                let lecturer = match lecturers.get(&lec_id) { Some(l) => l, None => { best_pos = pos; break; } };
                let batch = match input.batches.iter().find(|b| b.id == batch_id) { Some(b) => b, None => continue };
                // Use batch size for room capacity filter in domain_size
                let ds = domain_size(
                    batch_id, course, lecturer, &working_days,
                    &room_busy, &lecturer_busy, &batch_busy,
                    &lecturer_day_load, &lecturer_week_load, &lecturer_day_slots,
                    &rooms.iter().filter(|r| r.capacity >= batch.size).cloned().collect::<Vec<_>>(),
                );
                if ds < best_domain {
                    best_domain = ds;
                    best_pos = pos;
                }
            }
            best_pos
        };

        let need_idx = pending[chosen_pos];
        let (batch_id, course_id, hours_needed, is_biweekly) = needs[need_idx];

        let batch = match input.batches.iter().find(|b| b.id == batch_id) {
            Some(b) => b,
            None => { pending.remove(chosen_pos); continue; }
        };
        let course = match courses.get(&course_id) {
            Some(c) => c,
            None => { pending.remove(chosen_pos); continue; }
        };
        let lecturer_id = match course.lecturer_id {
            Some(lid) => lid,
            None => {
                unscheduled.push(UnscheduledItem {
                    batch_name: batch.name.clone(),
                    course_code: course.code.clone(),
                    course_name: course.name.clone(),
                    hours_needed,
                    reason: "No lecturer assigned to course".into(),
                });
                pending.remove(chosen_pos);
                continue;
            }
        };
        let lecturer = match lecturers.get(&lecturer_id) {
            Some(l) => l,
            None => {
                unscheduled.push(UnscheduledItem {
                    batch_name: batch.name.clone(),
                    course_code: course.code.clone(),
                    course_name: course.name.clone(),
                    hours_needed,
                    reason: "Assigned lecturer not found".into(),
                });
                pending.remove(chosen_pos);
                continue;
            }
        };

        let lec_avail_days: HashSet<&str> = lecturer.available_days.split(',').map(|s| s.trim()).collect();
        let class_type = &course.class_type;

        // ── Build and sort candidates with LCV tie-breaking ──────────────────
        let mut candidates: Vec<(&str, i64, i64)> = Vec::new(); // (day, slot, room_id)
        for &day in &working_days {
            if !lec_avail_days.contains(day) { continue; }
            let day_load = lecturer_day_load.get(&(lecturer_id, day.to_string())).copied().unwrap_or(0);
            if day_load >= lecturer.max_hours_per_day { continue; }
            let week_load = lecturer_week_load.get(&lecturer_id).copied().unwrap_or(0);
            if week_load >= lecturer.max_hours_per_week { continue; }

            let occupied = lecturer_day_slots.get(&(lecturer_id, day.to_string()))
                .map(|v| v.as_slice()).unwrap_or(&[]);

            for &slot in TIME_SLOTS {
                if is_blacked_out(&lecturer.blackout_json, day, slot) { continue; }
                if would_exceed_consecutive(occupied, slot, lecturer.max_consecutive_hours) { continue; }
                let ds = (day.to_string(), slot);
                if lecturer_busy.get(&lecturer_id).map_or(false, |s| s.contains(&ds)) { continue; }
                if batch_busy.get(&batch_id).map_or(false, |s| s.contains(&ds)) { continue; }
                // Find a suitable room
                let room = rooms.iter().find(|r| {
                    r.room_type == course.room_type
                        && r.capacity >= batch.size
                        && r.available_days.split(',').any(|d| d.trim() == day)
                        && !room_busy.get(&r.id).map_or(false, |s| s.contains(&ds))
                });
                if let Some(r) = room {
                    candidates.push((day, slot, r.id));
                }
            }
        }

        // Sort: (batch_day_count, slot_penalty + preferred, day_idx, slot, LCV cost)
        let remaining_for_lcv: Vec<(i64, i64, i64, bool)> = pending.iter()
            .filter(|&&i| i != need_idx)
            .map(|&i| needs[i])
            .collect();
        candidates.sort_by_key(|(day, slot, room_id)| {
            let bdc = *batch_day_count.get(&(batch_id, day.to_string())).unwrap_or(&0);
            let sp = slot_penalty(class_type, *slot);
            let pp = preferred_penalty(&lecturer.preferred_slots_json, day, *slot);
            let di = working_days.iter().position(|&d| d == *day).unwrap_or(0) as i64;
            let lcv = lcv_cost(
                day, *slot, *room_id, lecturer_id, batch_id,
                &remaining_for_lcv, &courses, &lecturers,
                &working_days, &room_busy, &lecturer_busy, &batch_busy,
            );
            (bdc, sp + pp, di, *slot, lcv)
        });

        // ── Place as many sessions as we can from candidates ─────────────────
        let mut placed = 0i64;
        for (day, slot, room_id) in &candidates {
            if placed >= hours_needed { break; }
            // Re-validate (state may have changed from earlier iterations of this loop)
            let ds = (day.to_string(), *slot);
            let week_load = lecturer_week_load.get(&lecturer_id).copied().unwrap_or(0);
            if week_load >= lecturer.max_hours_per_week { break; }
            let day_load = lecturer_day_load.get(&(lecturer_id, day.to_string())).copied().unwrap_or(0);
            if day_load >= lecturer.max_hours_per_day { continue; }
            if lecturer_busy.get(&lecturer_id).map_or(false, |s| s.contains(&ds)) { continue; }
            if batch_busy.get(&batch_id).map_or(false, |s| s.contains(&ds)) { continue; }
            if room_busy.get(room_id).map_or(false, |s| s.contains(&ds)) { continue; }
            let occupied = lecturer_day_slots.get(&(lecturer_id, day.to_string()))
                .map(|v| v.as_slice()).unwrap_or(&[]);
            if would_exceed_consecutive(occupied, *slot, lecturer.max_consecutive_hours) { continue; }

            room_busy.entry(*room_id).or_default().insert(ds.clone());
            lecturer_busy.entry(lecturer_id).or_default().insert(ds.clone());
            batch_busy.entry(batch_id).or_default().insert(ds.clone());
            *lecturer_day_load.entry((lecturer_id, day.to_string())).or_insert(0) += 1;
            *lecturer_week_load.entry(lecturer_id).or_insert(0) += 1;
            *batch_day_count.entry((batch_id, day.to_string())).or_insert(0) += 1;
            lecturer_day_slots.entry((lecturer_id, day.to_string())).or_default().push(*slot);

            entries.push(PlacedEntry {
                course_id,
                lecturer_id,
                room_id: *room_id,
                batch_id,
                day: day.to_string(),
                time_slot: *slot,
                class_type: class_type.clone(),
                week_parity: if is_biweekly { 1 } else { 0 },
            });
            placed += 1;
        }

        if placed >= hours_needed {
            pending.remove(chosen_pos);
            continue;
        }

        // ── Backjump recovery ─────────────────────────────────────────────────
        // Find the most recent placed entry whose lecturer or batch conflicts with
        // this need, and try to move it to an alternative slot.
        let mut recovered = false;
        if backjumps_used < MAX_BACKJUMPS {
            // Walk entries in reverse to find a candidate to move
            let conflict_pos = entries.iter().rposition(|e| {
                e.lecturer_id == lecturer_id
                    || e.batch_id == batch_id
            });

            if let Some(ci) = conflict_pos {
                let conflicting = entries[ci].clone();
                // Undo the conflicting placement
                let ds_old = (conflicting.day.clone(), conflicting.time_slot);
                room_busy.entry(conflicting.room_id).or_default().remove(&ds_old);
                lecturer_busy.entry(conflicting.lecturer_id).or_default().remove(&ds_old);
                batch_busy.entry(conflicting.batch_id).or_default().remove(&ds_old);
                *lecturer_day_load.entry((conflicting.lecturer_id, conflicting.day.clone())).or_insert(1) -= 1;
                *lecturer_week_load.entry(conflicting.lecturer_id).or_insert(1) -= 1;
                *batch_day_count.entry((conflicting.batch_id, conflicting.day.clone())).or_insert(1) -= 1;
                if let Some(v) = lecturer_day_slots.get_mut(&(conflicting.lecturer_id, conflicting.day.clone())) {
                    v.retain(|&s| s != conflicting.time_slot);
                }
                entries.remove(ci);
                backjumps_used += 1;

                // Put the conflicting need's original need back in pending
                // Find its need_idx by matching course/batch
                if let Some(orig_pos) = needs.iter().position(|(b, c, _, _)| {
                    *b == conflicting.batch_id && *c == conflicting.course_id
                }) {
                    if !pending.contains(&orig_pos) {
                        pending.push(orig_pos);
                    }
                }
                recovered = true;
            }
        }

        if !recovered {
            // Give up on remaining sessions for this need
            if placed < hours_needed {
                unscheduled.push(UnscheduledItem {
                    batch_name: batch.name.clone(),
                    course_code: course.code.clone(),
                    course_name: course.name.clone(),
                    hours_needed: hours_needed - placed,
                    reason: format!(
                        "Could only place {}/{} sessions — no valid slot/room for remaining",
                        placed, hours_needed
                    ),
                });
            }
            pending.remove(chosen_pos);
        }
        // If recovered, we stay in the loop and retry this need next iteration
    }

    ScheduleResult { entries, unscheduled }
}


// ══════════════════════════════════════════════════════════════════════════════
// TESTS
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Builder helpers ───────────────────────────────────────────────────────

    fn lec(id: i64, days: &str, max_day: i64, max_week: i64) -> Lecturer {
        Lecturer {
            id,
            name: format!("Lec{}", id),
            email: None,
            available_days: days.to_string(),
            max_hours_per_day: max_day,
            max_hours_per_week: max_week,
            org_id: Some(1),
            preferred_slots_json: None,
            blackout_json: None,
            max_consecutive_hours: 0,
        }
    }

    fn course(id: i64, hours: i64, rtype: &str, ctype: &str, lid: Option<i64>) -> Course {
        Course {
            id,
            code: format!("CS{:03}", id),
            name: format!("Course {}", id),
            hours_per_week: hours,
            room_type: rtype.to_string(),
            class_type: ctype.to_string(),
            frequency: "weekly".to_string(),
            lecturer_id: lid,
            lecturer_name: None,
            org_id: Some(1),
        }
    }

    fn room(id: i64, cap: i64, rtype: &str) -> Room {
        Room {
            id,
            name: format!("R{}", id),
            capacity: cap,
            room_type: rtype.to_string(),
            available_days: "Mon,Tue,Wed,Thu,Fri".to_string(),
            org_id: Some(1),
        }
    }

    fn batch(id: i64, size: i64, cids: Vec<i64>) -> Batch {
        Batch {
            id,
            name: format!("B{}", id),
            department: "CS".to_string(),
            semester: 1,
            size,
            course_ids: cids,
            org_id: Some(1),
            semester_id: None,
        }
    }

    fn simple() -> SchedulerInput {
        SchedulerInput {
            courses:      vec![course(1, 2, "lecture", "lecture", Some(1))],
            lecturers:    vec![lec(1, "Mon,Tue,Wed,Thu,Fri", 4, 20)],
            rooms:        vec![room(1, 30, "lecture")],
            batches:      vec![batch(1, 25, vec![1])],
            working_days: vec![],
        }
    }

    // Collect all (room, day, slot) occupancies and assert no duplicates
    fn assert_no_room_conflicts(entries: &[PlacedEntry]) {
        let mut seen = std::collections::HashSet::new();
        for e in entries {
            let key = (e.room_id, e.day.clone(), e.time_slot);
            assert!(!seen.contains(&key), "Room double-booked: {:?}", key);
            seen.insert(key);
        }
    }

    fn assert_no_lecturer_conflicts(entries: &[PlacedEntry]) {
        let mut seen = std::collections::HashSet::new();
        for e in entries {
            let key = (e.lecturer_id, e.day.clone(), e.time_slot);
            assert!(!seen.contains(&key), "Lecturer double-booked: {:?}", key);
            seen.insert(key);
        }
    }

    fn assert_no_batch_conflicts(entries: &[PlacedEntry]) {
        let mut seen = std::collections::HashSet::new();
        for e in entries {
            let key = (e.batch_id, e.day.clone(), e.time_slot);
            assert!(!seen.contains(&key), "Batch double-booked: {:?}", key);
            seen.insert(key);
        }
    }

    // ── slot_penalty ─────────────────────────────────────────────────────────

    #[test]
    fn slot_penalty_lab_prefers_afternoon() {
        assert_eq!(slot_penalty("lab", 0), 3);
        assert_eq!(slot_penalty("lab", 3), 3);
        assert_eq!(slot_penalty("lab", 4), 0);
        assert_eq!(slot_penalty("lab", 7), 0);
    }

    #[test]
    fn slot_penalty_tutorial_prefers_morning() {
        assert_eq!(slot_penalty("tutorial", 0), 0);
        assert_eq!(slot_penalty("tutorial", 3), 0);
        assert_eq!(slot_penalty("tutorial", 4), 2);
        assert_eq!(slot_penalty("tutorial", 7), 2);
    }

    #[test]
    fn slot_penalty_lecture_neutral_middle_slots() {
        assert_eq!(slot_penalty("lecture", 0), 1); // very early
        assert_eq!(slot_penalty("lecture", 7), 1); // very late
        assert_eq!(slot_penalty("lecture", 3), 0); // fine
        assert_eq!(slot_penalty("lecture", 4), 0); // fine
    }

    // ── preferred_penalty ────────────────────────────────────────────────────

    #[test]
    fn preferred_penalty_morning_met() {
        let p = Some(r#"{"Mon":"morning"}"#.to_string());
        assert_eq!(preferred_penalty(&p, "Mon", 2), 0);
    }

    #[test]
    fn preferred_penalty_morning_violated() {
        let p = Some(r#"{"Mon":"morning"}"#.to_string());
        assert_eq!(preferred_penalty(&p, "Mon", 5), 2);
    }

    #[test]
    fn preferred_penalty_afternoon_met() {
        let p = Some(r#"{"Tue":"afternoon"}"#.to_string());
        assert_eq!(preferred_penalty(&p, "Tue", 5), 0);
    }

    #[test]
    fn preferred_penalty_afternoon_violated() {
        let p = Some(r#"{"Tue":"afternoon"}"#.to_string());
        assert_eq!(preferred_penalty(&p, "Tue", 2), 2);
    }

    #[test]
    fn preferred_penalty_none_is_zero() {
        assert_eq!(preferred_penalty(&None, "Mon", 5), 0);
    }

    #[test]
    fn preferred_penalty_empty_string_is_zero() {
        let p = Some("".to_string());
        assert_eq!(preferred_penalty(&p, "Mon", 5), 0);
    }

    #[test]
    fn preferred_penalty_wrong_day_is_zero() {
        let p = Some(r#"{"Mon":"morning"}"#.to_string());
        assert_eq!(preferred_penalty(&p, "Fri", 7), 0);
    }

    // ── is_blacked_out ───────────────────────────────────────────────────────

    #[test]
    fn blackout_entire_day() {
        let b = Some(r#"[{"day":"Mon","slot":null}]"#.to_string());
        assert!(is_blacked_out(&b, "Mon", 0));
        assert!(is_blacked_out(&b, "Mon", 7));
        assert!(!is_blacked_out(&b, "Tue", 0));
    }

    #[test]
    fn blackout_specific_slot() {
        let b = Some(r#"[{"day":"Fri","slot":7}]"#.to_string());
        assert!(is_blacked_out(&b, "Fri", 7));
        assert!(!is_blacked_out(&b, "Fri", 6));
        assert!(!is_blacked_out(&b, "Mon", 7));
    }

    #[test]
    fn blackout_multiple_entries() {
        let b = Some(r#"[{"day":"Mon","slot":null},{"day":"Wed","slot":3}]"#.to_string());
        assert!(is_blacked_out(&b, "Mon", 5));
        assert!(is_blacked_out(&b, "Wed", 3));
        assert!(!is_blacked_out(&b, "Wed", 4));
        assert!(!is_blacked_out(&b, "Thu", 0));
    }

    #[test]
    fn blackout_empty_list_never_blocks() {
        let b = Some("[]".to_string());
        assert!(!is_blacked_out(&b, "Mon", 0));
    }

    #[test]
    fn blackout_none_never_blocks() {
        assert!(!is_blacked_out(&None, "Mon", 0));
    }

    // ── would_exceed_consecutive ─────────────────────────────────────────────

    #[test]
    fn consecutive_basic_exceeds() {
        // [1,2] + 3 = run of 3; max=2 → exceeds
        assert!(would_exceed_consecutive(&[1, 2], 3, 2));
    }

    #[test]
    fn consecutive_basic_allows_at_max() {
        // [0,1] + 2 = run of 3; max=3 → exactly at limit, allowed
        assert!(!would_exceed_consecutive(&[0, 1], 2, 3));
    }

    #[test]
    fn consecutive_exceeds_one_above_max() {
        // [0,1] + 2 = run of 3; max=2 → exceeds
        assert!(would_exceed_consecutive(&[0, 1], 2, 2));
    }

    #[test]
    fn consecutive_lunch_gap_breaks_run() {
        // Slots 3→4 = lunch gap (11:00–12:00 / 13:00–14:00), NOT consecutive
        // [1,2,3] + 4 → run of 1,2,3 = 3 and 4 alone; max=3 → no exceed
        assert!(!would_exceed_consecutive(&[1, 2, 3], 4, 3));
    }

    #[test]
    fn consecutive_after_lunch_creates_new_run() {
        // [3,4,5] → gap between 3 and 4, so runs are [3] and [4,5]
        // Adding slot 6: run becomes [4,5,6] = 3; max=2 → exceeds
        assert!(would_exceed_consecutive(&[3, 4, 5], 6, 2));
    }

    #[test]
    fn consecutive_max_zero_disabled() {
        // max_consecutive=0 means unlimited
        assert!(!would_exceed_consecutive(&[0, 1, 2, 3, 4, 5, 6], 7, 0));
    }

    #[test]
    fn consecutive_non_adjacent_slots_never_run() {
        // [0, 2, 4] are all separated; adding 6 still no runs
        assert!(!would_exceed_consecutive(&[0, 2, 4], 6, 2));
    }

    // ── Hard Constraint: room double-booking ─────────────────────────────────

    #[test]
    fn hard_no_room_double_booking() {
        // Two batches, two courses, one room — they cannot share (day, slot, room)
        let inp = SchedulerInput {
            courses:      vec![course(1, 1, "lecture", "lecture", Some(1)),
                               course(2, 1, "lecture", "lecture", Some(2))],
            lecturers:    vec![lec(1, "Mon,Tue,Wed,Thu,Fri", 4, 20),
                               lec(2, "Mon,Tue,Wed,Thu,Fri", 4, 20)],
            rooms:        vec![room(1, 30, "lecture")],
            batches:      vec![batch(1, 25, vec![1]), batch(2, 25, vec![2])],
            working_days: vec![],
        };
        let r = generate(&inp);
        assert_no_room_conflicts(&r.entries);
    }

    // ── Hard Constraint: lecturer double-booking ─────────────────────────────

    #[test]
    fn hard_no_lecturer_double_booking() {
        // Same lecturer, two batches, two rooms
        let inp = SchedulerInput {
            courses:      vec![course(1, 5, "lecture", "lecture", Some(1)),
                               course(2, 5, "lecture", "lecture", Some(1))],
            lecturers:    vec![lec(1, "Mon,Tue,Wed,Thu,Fri", 8, 40)],
            rooms:        vec![room(1, 30, "lecture"), room(2, 30, "lecture")],
            batches:      vec![batch(1, 25, vec![1]), batch(2, 25, vec![2])],
            working_days: vec![],
        };
        let r = generate(&inp);
        assert_no_lecturer_conflicts(&r.entries);
    }

    // ── Hard Constraint: batch double-booking ────────────────────────────────

    #[test]
    fn hard_no_batch_double_booking() {
        // Single batch, two courses — batch can't be two places at once
        let inp = SchedulerInput {
            courses:      vec![course(1, 5, "lecture", "lecture", Some(1)),
                               course(2, 5, "lecture", "lecture", Some(2))],
            lecturers:    vec![lec(1, "Mon,Tue,Wed,Thu,Fri", 8, 40),
                               lec(2, "Mon,Tue,Wed,Thu,Fri", 8, 40)],
            rooms:        vec![room(1, 30, "lecture"), room(2, 30, "lecture")],
            batches:      vec![batch(1, 25, vec![1, 2])],
            working_days: vec![],
        };
        let r = generate(&inp);
        assert_no_batch_conflicts(&r.entries);
    }

    // ── Hard Constraint: room type matching ──────────────────────────────────

    #[test]
    fn hard_lab_uses_only_lab_room() {
        let inp = SchedulerInput {
            courses:      vec![course(1, 2, "lab", "lab", Some(1))],
            lecturers:    vec![lec(1, "Mon,Tue,Wed,Thu,Fri", 4, 20)],
            rooms:        vec![room(1, 30, "lecture"), room(2, 30, "lab")],
            batches:      vec![batch(1, 25, vec![1])],
            working_days: vec![],
        };
        let r = generate(&inp);
        assert_eq!(r.entries.len(), 2);
        for e in &r.entries { assert_eq!(e.room_id, 2, "Lab must use lab room"); }
    }

    #[test]
    fn hard_lecture_never_uses_lab_room() {
        let inp = SchedulerInput {
            courses:      vec![course(1, 2, "lecture", "lecture", Some(1))],
            lecturers:    vec![lec(1, "Mon,Tue,Wed,Thu,Fri", 4, 20)],
            rooms:        vec![room(1, 30, "lab"), room(2, 30, "lecture")],
            batches:      vec![batch(1, 25, vec![1])],
            working_days: vec![],
        };
        let r = generate(&inp);
        assert_eq!(r.entries.len(), 2);
        for e in &r.entries { assert_eq!(e.room_id, 2, "Lecture must use lecture room"); }
    }

    #[test]
    fn hard_no_lab_room_causes_unscheduled() {
        let inp = SchedulerInput {
            courses:      vec![course(1, 2, "lab", "lab", Some(1))],
            lecturers:    vec![lec(1, "Mon,Tue,Wed,Thu,Fri", 4, 20)],
            rooms:        vec![room(1, 30, "lecture")],
            batches:      vec![batch(1, 25, vec![1])],
            working_days: vec![],
        };
        let r = generate(&inp);
        assert!(r.entries.is_empty());
        assert_eq!(r.unscheduled.len(), 1);
    }

    // ── Hard Constraint: room capacity ───────────────────────────────────────

    #[test]
    fn hard_room_capacity_picks_large_enough_room() {
        let inp = SchedulerInput {
            courses:      vec![course(1, 1, "lecture", "lecture", Some(1))],
            lecturers:    vec![lec(1, "Mon,Tue,Wed,Thu,Fri", 4, 20)],
            rooms:        vec![room(1, 10, "lecture"), room(2, 60, "lecture")],
            batches:      vec![batch(1, 50, vec![1])],
            working_days: vec![],
        };
        let r = generate(&inp);
        assert_eq!(r.entries.len(), 1);
        assert_eq!(r.entries[0].room_id, 2);
    }

    #[test]
    fn hard_room_too_small_unscheduled() {
        let inp = SchedulerInput {
            courses:      vec![course(1, 1, "lecture", "lecture", Some(1))],
            lecturers:    vec![lec(1, "Mon,Tue,Wed,Thu,Fri", 4, 20)],
            rooms:        vec![room(1, 10, "lecture")],
            batches:      vec![batch(1, 100, vec![1])],
            working_days: vec![],
        };
        let r = generate(&inp);
        assert!(r.entries.is_empty());
        assert_eq!(r.unscheduled.len(), 1);
    }

    // ── Hard Constraint: lecturer availability ───────────────────────────────

    #[test]
    fn hard_lecturer_only_placed_on_available_days() {
        let inp = SchedulerInput {
            courses:      vec![course(1, 3, "lecture", "lecture", Some(1))],
            lecturers:    vec![Lecturer {
                id: 1, name: "Mon-only".to_string(), email: None,
                available_days: "Mon".to_string(),
                max_hours_per_day: 8, max_hours_per_week: 20,
                org_id: Some(1), preferred_slots_json: None, blackout_json: None,
                max_consecutive_hours: 0,
            }],
            rooms:        vec![room(1, 30, "lecture")],
            batches:      vec![batch(1, 25, vec![1])],
            working_days: vec![],
        };
        let r = generate(&inp);
        assert_eq!(r.entries.len(), 3);
        for e in &r.entries { assert_eq!(e.day, "Mon"); }
    }

    #[test]
    fn hard_lecturer_no_available_days_unscheduled() {
        let inp = SchedulerInput {
            courses:      vec![course(1, 1, "lecture", "lecture", Some(1))],
            lecturers:    vec![Lecturer {
                id: 1, name: "Unavailable".to_string(), email: None,
                available_days: "".to_string(),
                max_hours_per_day: 8, max_hours_per_week: 20,
                org_id: Some(1), preferred_slots_json: None, blackout_json: None,
                max_consecutive_hours: 0,
            }],
            rooms:        vec![room(1, 30, "lecture")],
            batches:      vec![batch(1, 25, vec![1])],
            working_days: vec![],
        };
        let r = generate(&inp);
        assert!(r.entries.is_empty());
    }

    // ── Hard Constraint: max_hours_per_day ───────────────────────────────────

    #[test]
    fn hard_max_hours_per_day_not_exceeded() {
        let inp = SchedulerInput {
            courses:      vec![course(1, 8, "lecture", "lecture", Some(1))],
            lecturers:    vec![Lecturer {
                id: 1, name: "L".to_string(), email: None,
                available_days: "Mon,Tue,Wed,Thu,Fri".to_string(),
                max_hours_per_day: 2,
                max_hours_per_week: 40,
                org_id: Some(1), preferred_slots_json: None, blackout_json: None,
                max_consecutive_hours: 0,
            }],
            rooms:        vec![room(1, 30, "lecture")],
            batches:      vec![batch(1, 25, vec![1])],
            working_days: vec![],
        };
        let r = generate(&inp);
        let mut day_count: std::collections::HashMap<String, i64> = Default::default();
        for e in &r.entries { *day_count.entry(e.day.clone()).or_insert(0) += 1; }
        for (day, cnt) in &day_count {
            assert!(*cnt <= 2, "max_hours_per_day exceeded on {}: {}", day, cnt);
        }
    }

    // ── Hard Constraint: max_hours_per_week ──────────────────────────────────

    #[test]
    fn hard_max_hours_per_week_caps_placement() {
        let inp = SchedulerInput {
            courses:      vec![course(1, 10, "lecture", "lecture", Some(1))],
            lecturers:    vec![Lecturer {
                id: 1, name: "L".to_string(), email: None,
                available_days: "Mon,Tue,Wed,Thu,Fri".to_string(),
                max_hours_per_day: 8,
                max_hours_per_week: 5,
                org_id: Some(1), preferred_slots_json: None, blackout_json: None,
                max_consecutive_hours: 0,
            }],
            rooms:        vec![room(1, 30, "lecture")],
            batches:      vec![batch(1, 25, vec![1])],
            working_days: vec![],
        };
        let r = generate(&inp);
        assert_eq!(r.entries.len(), 5, "exactly max_hours_per_week sessions placed");
        assert_eq!(r.unscheduled.len(), 1);
        assert!(r.unscheduled[0].reason.contains("weekly max"));
    }

    // ── Hard Constraint: blackout slots ──────────────────────────────────────

    #[test]
    fn hard_blackout_days_avoided() {
        // Blackout Mon–Thu → all sessions must land on Fri
        let inp = SchedulerInput {
            courses:      vec![course(1, 3, "lecture", "lecture", Some(1))],
            lecturers:    vec![Lecturer {
                id: 1, name: "L".to_string(), email: None,
                available_days: "Mon,Tue,Wed,Thu,Fri".to_string(),
                max_hours_per_day: 8, max_hours_per_week: 20,
                org_id: Some(1),
                preferred_slots_json: None,
                blackout_json: Some(r#"[
                    {"day":"Mon","slot":null},{"day":"Tue","slot":null},
                    {"day":"Wed","slot":null},{"day":"Thu","slot":null}
                ]"#.to_string()),
                max_consecutive_hours: 0,
            }],
            rooms:        vec![room(1, 30, "lecture")],
            batches:      vec![batch(1, 25, vec![1])],
            working_days: vec![],
        };
        let r = generate(&inp);
        for e in &r.entries { assert_eq!(e.day, "Fri", "Blackout days must be avoided"); }
    }

    #[test]
    fn hard_blackout_specific_slot_avoided() {
        // Blackout Mon slot 0 only
        let inp = SchedulerInput {
            courses:      vec![course(1, 1, "lecture", "lecture", Some(1))],
            lecturers:    vec![Lecturer {
                id: 1, name: "L".to_string(), email: None,
                available_days: "Mon".to_string(),
                max_hours_per_day: 8, max_hours_per_week: 20,
                org_id: Some(1),
                preferred_slots_json: None,
                blackout_json: Some(r#"[{"day":"Mon","slot":0}]"#.to_string()),
                max_consecutive_hours: 0,
            }],
            rooms:        vec![room(1, 30, "lecture")],
            batches:      vec![batch(1, 25, vec![1])],
            working_days: vec![],
        };
        let r = generate(&inp);
        assert_eq!(r.entries.len(), 1);
        assert_ne!(r.entries[0].time_slot, 0, "Blacked-out slot 0 must not be used");
    }

    // ── Hard Constraint: max_consecutive_hours ───────────────────────────────

    #[test]
    fn hard_max_consecutive_hours_not_exceeded() {
        // Force everything to one day; max_consecutive=2 → no 3-in-a-row
        let inp = SchedulerInput {
            courses:      vec![course(1, 6, "lecture", "lecture", Some(1))],
            lecturers:    vec![Lecturer {
                id: 1, name: "L".to_string(), email: None,
                available_days: "Mon".to_string(),
                max_hours_per_day: 8, max_hours_per_week: 40,
                org_id: Some(1),
                preferred_slots_json: None, blackout_json: None,
                max_consecutive_hours: 2,
            }],
            rooms:        vec![room(1, 30, "lecture")],
            batches:      vec![batch(1, 25, vec![1])],
            working_days: vec![],
        };
        let r = generate(&inp);
        let mut slots: Vec<i64> = r.entries.iter()
            .filter(|e| e.day == "Mon")
            .map(|e| e.time_slot)
            .collect();
        slots.sort_unstable();
        let mut run = 1i64;
        for i in 1..slots.len() {
            let (a, b) = (slots[i-1], slots[i]);
            if b == a + 1 && !(a == 3 && b == 4) {
                run += 1;
                assert!(run <= 2, "Consecutive run {} > max_consecutive_hours=2", run);
            } else {
                run = 1;
            }
        }
    }

    #[test]
    fn hard_lunch_gap_not_treated_as_consecutive() {
        // Slots 3 and 4 are NOT consecutive (lunch gap)
        // Ensure would_exceed_consecutive([3], 4, 1) returns false
        assert!(!would_exceed_consecutive(&[3], 4, 1),
            "Slots 3 and 4 cross lunch gap and must not be treated as consecutive");
    }

    // ── Unscheduled reporting ─────────────────────────────────────────────────

    #[test]
    fn unscheduled_no_lecturer() {
        let inp = SchedulerInput {
            courses:      vec![course(1, 2, "lecture", "lecture", None)],
            lecturers:    vec![],
            rooms:        vec![room(1, 30, "lecture")],
            batches:      vec![batch(1, 25, vec![1])],
            working_days: vec![],
        };
        let r = generate(&inp);
        assert!(r.entries.is_empty());
        assert_eq!(r.unscheduled.len(), 1);
        assert!(r.unscheduled[0].reason.contains("No lecturer"));
    }

    #[test]
    fn unscheduled_missing_lecturer_record() {
        // Lecturer ID 99 referenced but not provided
        let inp = SchedulerInput {
            courses:      vec![course(1, 1, "lecture", "lecture", Some(99))],
            lecturers:    vec![],
            rooms:        vec![room(1, 30, "lecture")],
            batches:      vec![batch(1, 25, vec![1])],
            working_days: vec![],
        };
        let r = generate(&inp);
        assert!(r.entries.is_empty());
        assert_eq!(r.unscheduled.len(), 1);
        assert!(r.unscheduled[0].reason.contains("not found"));
    }

    #[test]
    fn unscheduled_partial_placement_reports_remaining() {
        // 10 hours needed but weekly cap = 6 → 4 unscheduled hours
        let inp = SchedulerInput {
            courses:      vec![course(1, 10, "lecture", "lecture", Some(1))],
            lecturers:    vec![Lecturer {
                id: 1, name: "L".to_string(), email: None,
                available_days: "Mon,Tue,Wed,Thu,Fri".to_string(),
                max_hours_per_day: 8, max_hours_per_week: 6,
                org_id: Some(1), preferred_slots_json: None, blackout_json: None,
                max_consecutive_hours: 0,
            }],
            rooms:        vec![room(1, 30, "lecture")],
            batches:      vec![batch(1, 25, vec![1])],
            working_days: vec![],
        };
        let r = generate(&inp);
        assert_eq!(r.entries.len(), 6);
        assert_eq!(r.unscheduled.len(), 1);
        assert_eq!(r.unscheduled[0].hours_needed, 4,
            "Should report exactly 4 remaining unscheduled hours");
    }

    // ── Empty / trivial inputs ────────────────────────────────────────────────

    #[test]
    fn empty_input_produces_empty_output() {
        let r = generate(&SchedulerInput {
            courses: vec![], lecturers: vec![], rooms: vec![], batches: vec![], working_days: vec![],
        });
        assert!(r.entries.is_empty());
        assert!(r.unscheduled.is_empty());
    }

    #[test]
    fn all_sessions_placed_for_trivial_case() {
        let r = generate(&simple());
        assert_eq!(r.entries.len(), 2);
        assert!(r.unscheduled.is_empty());
    }

    // ── Biweekly ─────────────────────────────────────────────────────────────

    fn biweekly_course(id: i64, hrs: i64, lid: i64) -> Course {
        Course {
            id, code: format!("BW{}", id), name: format!("Biweekly {}", id),
            hours_per_week: hrs, room_type: "lecture".to_string(),
            class_type: "lecture".to_string(), frequency: "biweekly".to_string(),
            lecturer_id: Some(lid), lecturer_name: None, org_id: Some(1),
        }
    }

    #[test]
    fn biweekly_places_half_sessions_ceil() {
        // 4 hrs/week biweekly → ceil(4/2) = 2 sessions
        let inp = SchedulerInput {
            courses:      vec![biweekly_course(1, 4, 1)],
            lecturers:    vec![lec(1, "Mon,Tue,Wed,Thu,Fri", 8, 20)],
            rooms:        vec![room(1, 30, "lecture")],
            batches:      vec![batch(1, 25, vec![1])],
            working_days: vec![],
        };
        let r = generate(&inp);
        assert_eq!(r.entries.len(), 2, "biweekly 4hr → 2 sessions placed");
    }

    #[test]
    fn biweekly_odd_hours_ceil() {
        // 3 hrs/week biweekly → ceil(3/2) = 2 sessions
        let inp = SchedulerInput {
            courses:      vec![biweekly_course(1, 3, 1)],
            lecturers:    vec![lec(1, "Mon,Tue,Wed,Thu,Fri", 8, 20)],
            rooms:        vec![room(1, 30, "lecture")],
            batches:      vec![batch(1, 25, vec![1])],
            working_days: vec![],
        };
        let r = generate(&inp);
        assert_eq!(r.entries.len(), 2, "ceil(3/2) = 2 sessions");
    }

    #[test]
    fn biweekly_entries_have_week_parity_one() {
        let inp = SchedulerInput {
            courses:      vec![biweekly_course(1, 2, 1)],
            lecturers:    vec![lec(1, "Mon,Tue,Wed,Thu,Fri", 8, 20)],
            rooms:        vec![room(1, 30, "lecture")],
            batches:      vec![batch(1, 25, vec![1])],
            working_days: vec![],
        };
        let r = generate(&inp);
        for e in &r.entries {
            assert_eq!(e.week_parity, 1, "biweekly → week_parity must be 1");
        }
    }

    #[test]
    fn weekly_entries_have_week_parity_zero() {
        let r = generate(&simple());
        for e in &r.entries {
            assert_eq!(e.week_parity, 0, "weekly → week_parity must be 0");
        }
    }

    // ── Diversity heuristics ─────────────────────────────────────────────────

    #[test]
    fn diversity_spreads_batch_across_days() {
        // 5 independent 1-hr courses for one batch → expect ≥ 3 distinct days
        let courses: Vec<Course> = (1..=5).map(|i| course(i, 1, "lecture", "lecture", Some(i))).collect();
        let lecturers: Vec<Lecturer> = (1..=5).map(|i| lec(i, "Mon,Tue,Wed,Thu,Fri", 4, 20)).collect();
        let rooms: Vec<Room> = (1..=5).map(|i| room(i, 30, "lecture")).collect();
        let inp = SchedulerInput {
            courses, lecturers, rooms,
            batches:      vec![batch(1, 25, vec![1, 2, 3, 4, 5])],
            working_days: vec![],
        };
        let r = generate(&inp);
        assert_eq!(r.entries.len(), 5);
        let days: std::collections::HashSet<String> = r.entries.iter().map(|e| e.day.clone()).collect();
        assert!(days.len() >= 3, "Diversity: expected ≥3 days, got {:?}", days);
    }

    #[test]
    fn labs_prefer_afternoon_slots() {
        let inp = SchedulerInput {
            courses:      vec![course(1, 3, "lab", "lab", Some(1))],
            lecturers:    vec![lec(1, "Mon,Tue,Wed,Thu,Fri", 4, 20)],
            rooms:        vec![room(1, 30, "lab")],
            batches:      vec![batch(1, 25, vec![1])],
            working_days: vec![],
        };
        let r = generate(&inp);
        assert_eq!(r.entries.len(), 3);
        for e in &r.entries {
            assert!(e.time_slot >= 4, "Lab should be in afternoon (slot≥4), got {}", e.time_slot);
        }
    }

    #[test]
    fn tutorials_prefer_morning_slots() {
        let inp = SchedulerInput {
            courses:      vec![course(1, 3, "lecture", "tutorial", Some(1))],
            lecturers:    vec![lec(1, "Mon,Tue,Wed,Thu,Fri", 4, 20)],
            rooms:        vec![room(1, 30, "lecture")],
            batches:      vec![batch(1, 25, vec![1])],
            working_days: vec![],
        };
        let r = generate(&inp);
        assert_eq!(r.entries.len(), 3);
        for e in &r.entries {
            assert!(e.time_slot < 4, "Tutorial should be in morning (slot<4), got {}", e.time_slot);
        }
    }

    // ── Multi-entity consistency ──────────────────────────────────────────────

    #[test]
    fn multiple_batches_same_course_independent_sessions() {
        // Two batches both enrolled in course 1 → 4 total sessions (2 per batch)
        let inp = SchedulerInput {
            courses:      vec![course(1, 2, "lecture", "lecture", Some(1))],
            lecturers:    vec![lec(1, "Mon,Tue,Wed,Thu,Fri", 8, 40)],
            rooms:        vec![room(1, 30, "lecture"), room(2, 30, "lecture")],
            batches:      vec![batch(1, 25, vec![1]), batch(2, 25, vec![1])],
            working_days: vec![],
        };
        let r = generate(&inp);
        assert_eq!(r.entries.len(), 4, "2 batches × 2 hrs = 4 entries");
        assert_no_lecturer_conflicts(&r.entries);
        assert_no_batch_conflicts(&r.entries);
    }

    #[test]
    fn large_dataset_all_hard_constraints_hold() {
        // 10 lecturers, 20 courses, 10 rooms, 10 batches
        let lecturers: Vec<Lecturer> = (1..=10).map(|i| lec(i, "Mon,Tue,Wed,Thu,Fri", 4, 20)).collect();
        let courses: Vec<Course> = (1..=20).map(|i| course(i, 2, "lecture", "lecture", Some((i % 10) + 1))).collect();
        let rooms: Vec<Room> = (1..=10).map(|i| room(i, 40, "lecture")).collect();
        let batches: Vec<Batch> = (1..=10).map(|i| {
            batch(i, 30, vec![(i * 2 - 1).min(20), (i * 2).min(20)])
        }).collect();
        let r = generate(&SchedulerInput { courses, lecturers, rooms, batches, working_days: vec![] });
        assert_no_room_conflicts(&r.entries);
        assert_no_lecturer_conflicts(&r.entries);
        assert_no_batch_conflicts(&r.entries);
    }
}
