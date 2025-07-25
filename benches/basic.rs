// Copyright 2024 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use criterion::{BatchSize, BenchmarkId, Criterion};
use flyweights::FlyStr;
use foldhash::fast::FixedState;
use std::hash::{BuildHasher as _, Hash, Hasher};
use std::hint::black_box;

const INPUTS: &[&str; 6] = &[
    // Empty
    "",
    // Extra-small
    "1",
    // Largest inline
    "1234567",
    // Smallest heap
    "12345678",
    // Larger heap
    "123456789123456789123456789123456789123456789123456789123456789123456789123456789123456789",
    // Really big heap
    concat!(
        "123456789123456789123456789123456789123456789123456789123456789123456789123456789",
        "123456789123456789123456789123456789123456789123456789123456789123456789123456789",
        "123456789123456789123456789123456789123456789123456789123456789123456789123456789",
        "123456789123456789123456789123456789123456789123456789123456789123456789123456789",
        "123456789123456789123456789123456789123456789123456789123456789123456789123456789",
        "123456789123456789123456789123456789123456789123456789123456789123456789123456789",
        "123456789123456789123456789123456789123456789123456789123456789123456789123456789",
        "123456789123456789123456789123456789123456789123456789123456789123456789123456789",
        "123456789123456789123456789123456789123456789123456789123456789123456789123456789",
        "123456789123456789123456789123456789123456789123456789123456789123456789123456789",
        "123456789123456789123456789123456789123456789123456789123456789123456789123456789",
        "123456789123456789123456789123456789123456789123456789123456789123456789123456789",
    ),
];

#[cfg(not(miri))]
criterion::criterion_main!(
    lifecycle,
    access,
    hashing,
    comparison,
    multithreaded_contended,
    multithreaded_distributed,
);

// Criterion doesn't work in miri right now: https://github.com/bheisler/criterion.rs/issues/778
#[cfg(miri)]
fn main() {}

macro_rules! bench_over_inputs {
    ($name:ident, |$bencher:ident, $input:ident| $bench:expr) => {
        fn $name(c: &mut Criterion) {
            let mut group = c.benchmark_group(stringify!($name));
            group.plot_config(
                criterion::PlotConfiguration::default()
                    .summary_scale(criterion::AxisScale::Logarithmic),
            );
            for input in INPUTS {
                group.bench_with_input(
                    BenchmarkId::from_parameter(input.len()),
                    input,
                    |$bencher, input: &&str| {
                        let $input = black_box(*input);
                        $bench;
                    },
                );
            }
            group.finish();
        }
    };
}

criterion::criterion_group!(
    lifecycle,
    from_str_first,
    from_str_dupe,
    clone,
    drop_dupe,
    drop_last
);

bench_over_inputs!(from_str_first, |b, input| b.iter(|| FlyStr::from(input)));

bench_over_inputs!(from_str_dupe, |b, input| {
    // Make a copy of the FlyStr that will be live the whole time.
    let _existing = FlyStr::from(input);
    b.iter(|| FlyStr::from(input));
});

bench_over_inputs!(clone, |b, input| {
    let first = black_box(FlyStr::from(input));
    b.iter(|| first.clone());
});

bench_over_inputs!(drop_dupe, |b, input| {
    // Make a copy of the FlyStr that will be live the whole time so we drop a copy.
    let input = black_box(FlyStr::from(input));
    b.iter_batched(|| input.clone(), drop, BatchSize::PerIteration);
});

bench_over_inputs!(drop_last, |b, input| {
    b.iter_batched(|| FlyStr::from(input), drop, BatchSize::PerIteration);
});

criterion::criterion_group!(access, as_str);

bench_over_inputs!(as_str, |b, input| {
    let input = black_box(FlyStr::from(input));
    b.iter(|| input.as_str());
});

criterion::criterion_group!(hashing, foldhash);

bench_over_inputs!(foldhash, |b, input| {
    let input = black_box(FlyStr::from(input));
    b.iter_batched_ref(
        || FixedState::default().build_hasher(),
        |hasher| {
            input.hash(hasher);
            hasher.finish()
        },
        BatchSize::PerIteration,
    );
});

criterion::criterion_group!(comparison, eq);

bench_over_inputs!(eq, |b, input| {
    let first = black_box(FlyStr::from(input));
    let second = black_box(first.clone());
    b.iter(|| first == second);
});

macro_rules! contended_from_str {
    ($([$name:ident, $thread_count:expr];)+) => {
        criterion::criterion_group!(
            multithreaded_contended,
            $($name),+
        );

        $(
            bench_over_inputs!($name, |b, input| {
                std::thread::scope(|s| {
                    b.iter(|| {
                        let handles = std::array::from_fn::<_, $thread_count, _>(|_| {
                            s.spawn(|| FlyStr::from(input))
                        });

                        handles.map(|handle| handle.join().unwrap())
                    });
                })
            });
        )+
    }
}

contended_from_str! {
    [from_str_contended1, 1];
    [from_str_contended2, 2];
    [from_str_contended4, 4];
    [from_str_contended8, 8];
    [from_str_contended16, 16];
    [from_str_contended32, 32];
    [from_str_contended64, 64];
}

macro_rules! distributed_from_str {
    ($([$name:ident, $thread_count:expr];)+) => {
        criterion::criterion_group!(
            multithreaded_distributed,
            $($name),+
        );

        $(
            bench_over_inputs!($name, |b, input| {
                let inputs = std::array::from_fn::<_, {$thread_count * 10}, _>(|i| format!("{input}:{i}"));

                std::thread::scope(|s| {
                    let inputs = &inputs;
                    b.iter(|| {
                        let handles: [_; $thread_count] = std::array::from_fn(|thread_number| {
                            let thread_number: usize = thread_number;
                            s.spawn(move || {
                                std::array::from_fn::<_, 10, _>(|i| FlyStr::from(inputs[thread_number * i].as_str()))
                            })
                        });

                        handles.map(|handle| handle.join().unwrap())
                    });
                })
            });
        )+
    }
}

distributed_from_str! {
    [from_str_distributed1, 1];
    [from_str_distributed2, 2];
    [from_str_distributed4, 4];
    [from_str_distributed8, 8];
    [from_str_distributed16, 16];
    [from_str_distributed32, 32];
    [from_str_distributed64, 64];
}
