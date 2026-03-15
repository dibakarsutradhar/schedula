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
        let mut candidates: Vec<(&str, i64)> = DAYS
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
            let di = DAYS.iter().position(|&d| d == *day).unwrap_or(0) as i64;
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
