use std::ops::Range;

use anyhow::bail;
use plotters::prelude::*;

mod read_latency;
pub use read_latency::*;
mod profile;
pub use profile::*;

pub fn analysis_with_profile(profile: &Profile) -> anyhow::Result<()> {
    println!("analysis {} begin", profile.name);
    match profile.kind {
        ProfileKind::RandomReadLatency => {
            let pattern = crate::latency::Pattern::Random;
            let res = read_latency(
                &profile.group_size,
                profile.working_set.clone(),
                profile.round,
                pattern,
            )?;
            plot_read_latency(res, &profile.name, "group_size")?;
        }
        ProfileKind::RandomBlockReadLatency => {
            let Some(block_count_list) = &profile.block_count else {
                bail!("RandomBlockReadLatency 必须指定 block_count 参数")
            };
            if profile.group_size.len() != 1 {
                bail!("RandomBlockReadLatency 只能制定一个group_size")
            }
            let group_size = profile.group_size[0];
            let res = random_block_read_latency(
                &block_count_list,
                profile.working_set.clone(),
                group_size,
                profile.round,
            )?;
            plot_read_latency(res, &profile.name, "block_count")?;
        }
        ProfileKind::SeqReadLatency => {
            let pattern = crate::latency::Pattern::Seq;
            let res = read_latency(
                &profile.group_size,
                profile.working_set.clone(),
                profile.round,
                pattern,
            )?;
            plot_read_latency(res, &profile.name, "group_size")?;
        }
    }
    println!("analysis {} success", profile.name);
    Ok(())
}

pub struct ReadLatencyLine {
    pub legend: usize,
    pub avg_cycles: Vec<u64>,
}

pub struct ReadLatency {
    working_set_range: Range<u32>,
    latencies: Vec<ReadLatencyLine>,
    max_latency: u64,
}

static COLORS: &[RGBColor] = &[RED, BLACK, BLUE, GREEN, MAGENTA, YELLOW, CYAN];

pub fn plot_read_latency(input: ReadLatency, name: &str, legend: &str) -> anyhow::Result<()> {
    let working_set_range = input.working_set_range;
    let latencies = input.latencies;
    let max_latency = input.max_latency;

    if latencies.len() > COLORS.len() {
        bail!("not enough color")
    }
    let output = format!("{name}.png");
    let root = BitMapBackend::new(&output, (1024, 768)).into_drawing_area();
    root.fill(&WHITE)?;

    let font = ("Arial", 23).into_font();
    let mut chart = ChartBuilder::on(&root)
        .caption(name, font.clone())
        .x_label_area_size(50)
        .y_label_area_size(50)
        .margin_right(20)
        .build_cartesian_2d(working_set_range.clone(), 0..max_latency)?;

    chart
        .configure_mesh()
        .disable_x_mesh()
        .x_desc("Working Set Size")
        .label_style(font.clone())
        .y_desc("Cycles/Group")
        .draw()?;

    for (latency, color) in latencies.iter().zip(COLORS.iter()) {
        let pairs = working_set_range.clone().zip(latency.avg_cycles.clone());
        let points = PointSeries::of_element(pairs.clone(), 5, *color, &|c, s, st| {
            EmptyElement::at(c)    // We want to construct a composed element on-the-fly
            + Circle::new((0,0),s,st.filled()) // At this point, the new pixel coordinate is established
        });

        chart.draw_series(points)?;
        let line = LineSeries::new(pairs, color);
        let label = format!("{}={}", legend, latency.legend);
        chart
            .draw_series(line)
            .unwrap()
            .label(label)
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], *color));
    }

    chart
        .configure_series_labels()
        .label_font(font)
        .background_style(&WHITE)
        .border_style(&BLACK)
        .position(SeriesLabelPosition::UpperLeft)
        .draw()?;

    Ok(())
}
