/// Criterion benchmarks for the scheduler algorithm.
///
/// Run with:
///   cd src-tauri && cargo bench
///
/// Profiles:
///   tiny    —  5 lecturers,  10 courses, 3 rooms,  5 batches   (baseline)
///   small   — 10 lecturers,  20 courses, 5 rooms,  8 batches
///   medium  — 20 lecturers,  50 courses, 10 rooms, 15 batches  (typical university dept)
///   large   — 50 lecturers, 100 courses, 20 rooms, 30 batches  (large faculty)
///   stress  — 100 lecturers, 200 courses, 40 rooms, 60 batches (stress test)
///
/// Expected times (M2 Mac, rough targets):
///   tiny    <  1 ms
///   small   <  5 ms
///   medium  < 20 ms
///   large   < 80 ms
///   stress  < 300 ms

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use app_lib::{
    models::{Batch, Course, Lecturer, Room},
    scheduler::{generate, SchedulerInput},
};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn make_lec(id: i64, max_day: i64, max_week: i64) -> Lecturer {
    Lecturer {
        id,
        name: format!("Lec{}", id),
        email: None,
        available_days: "Mon,Tue,Wed,Thu,Fri".to_string(),
        max_hours_per_day: max_day,
        max_hours_per_week: max_week,
        org_id: Some(1),
        preferred_slots_json: None,
        blackout_json: None,
        max_consecutive_hours: 0,
    }
}

fn make_course(id: i64, hours: i64, lec_id: i64) -> Course {
    Course {
        id,
        code: format!("CS{:04}", id),
        name: format!("Course {}", id),
        hours_per_week: hours,
        room_type: "lecture".to_string(),
        class_type: "lecture".to_string(),
        frequency: "weekly".to_string(),
        lecturer_id: Some(lec_id),
        lecturer_name: None,
        org_id: Some(1),
    }
}

fn make_room(id: i64, cap: i64) -> Room {
    Room {
        id,
        name: format!("R{}", id),
        capacity: cap,
        room_type: "lecture".to_string(),
        available_days: "Mon,Tue,Wed,Thu,Fri".to_string(),
        org_id: Some(1),
    }
}

fn make_batch(id: i64, course_ids: Vec<i64>) -> Batch {
    Batch {
        id,
        name: format!("B{}", id),
        department: "CS".to_string(),
        semester: 1,
        size: 30,
        course_ids,
        org_id: Some(1),
        semester_id: None,
    }
}

/// Build an input with the given scale.
/// `lec_count` lecturers each teach `courses_per_lec` courses, split across `batch_count` batches.
fn build_input(lec_count: i64, courses_per_lec: i64, room_count: i64, batch_count: i64) -> SchedulerInput {
    let lecturers: Vec<Lecturer> = (1..=lec_count)
        .map(|i| make_lec(i, 4, 20))
        .collect();

    let total_courses = lec_count * courses_per_lec;
    let courses: Vec<Course> = (1..=total_courses)
        .map(|i| make_course(i, 2, (i % lec_count) + 1))
        .collect();

    let rooms: Vec<Room> = (1..=room_count)
        .map(|i| make_room(i, 50))
        .collect();

    // Distribute courses evenly across batches
    let batches: Vec<Batch> = (1..=batch_count)
        .map(|i| {
            // Each batch gets a slice of courses
            let start = ((i - 1) * total_courses / batch_count + 1).min(total_courses);
            let end = (i * total_courses / batch_count).min(total_courses);
            let cids: Vec<i64> = if start <= end { (start..=end).collect() } else { vec![] };
            make_batch(i, cids)
        })
        .collect();

    SchedulerInput { courses, lecturers, rooms, batches }
}

// ── Benchmarks ───────────────────────────────────────────────────────────────

fn bench_scheduler_scales(c: &mut Criterion) {
    let mut group = c.benchmark_group("scheduler");
    group.sample_size(20); // fewer samples for slow cases

    let profiles = [
        ("tiny",   5,   2,  3,  5),
        ("small",  10,  2,  5,  8),
        ("medium", 20,  3, 10, 15),
        ("large",  50,  2, 20, 30),
        ("stress", 100, 2, 40, 60),
    ];

    for (name, lecs, courses_per_lec, rooms, batches) in &profiles {
        group.bench_with_input(
            BenchmarkId::new("generate", name),
            name,
            |b, _| {
                let input = build_input(*lecs, *courses_per_lec, *rooms, *batches);
                b.iter(|| generate(black_box(&input)));
            },
        );
    }

    group.finish();
}

fn bench_constraint_helpers(c: &mut Criterion) {
    let mut group = c.benchmark_group("constraint_helpers");

    // Benchmark slot_penalty called many times (tight inner loop)
    group.bench_function("slot_penalty_1000", |b| {
        b.iter(|| {
            let mut sum = 0i64;
            for slot in 0..8 {
                for ct in &["lecture", "lab", "tutorial"] {
                    sum += app_lib::scheduler::slot_penalty_pub(black_box(ct), black_box(slot));
                }
            }
            sum
        });
    });

    // Benchmark preferred_penalty with JSON parsing
    let pref = Some(r#"{"Mon":"morning","Tue":"afternoon","Wed":"morning"}"#.to_string());
    group.bench_function("preferred_penalty_1000", |b| {
        b.iter(|| {
            let mut sum = 0i64;
            for slot in 0..8 {
                for day in &["Mon", "Tue", "Wed", "Thu", "Fri"] {
                    sum += app_lib::scheduler::preferred_penalty_pub(black_box(&pref), black_box(day), black_box(slot));
                }
            }
            sum
        });
    });

    // Benchmark blackout check
    let blackout = Some(r#"[{"day":"Mon","slot":null},{"day":"Wed","slot":3},{"day":"Fri","slot":7}]"#.to_string());
    group.bench_function("blackout_check_1000", |b| {
        b.iter(|| {
            let mut hits = 0u32;
            for slot in 0..8 {
                for day in &["Mon", "Tue", "Wed", "Thu", "Fri"] {
                    if app_lib::scheduler::is_blacked_out_pub(black_box(&blackout), black_box(day), black_box(slot)) {
                        hits += 1;
                    }
                }
            }
            hits
        });
    });

    group.finish();
}

fn bench_input_construction(c: &mut Criterion) {
    // Measure how long just building the input data takes (subtract from generate times)
    c.bench_function("build_medium_input", |b| {
        b.iter(|| build_input(black_box(20), black_box(3), black_box(10), black_box(15)));
    });
}

criterion_group!(
    benches,
    bench_scheduler_scales,
    bench_constraint_helpers,
    bench_input_construction,
);
criterion_main!(benches);
