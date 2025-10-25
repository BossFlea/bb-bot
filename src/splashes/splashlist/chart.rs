use anyhow::{Context as _, Result};
use image::{ImageBuffer, ImageFormat, Rgba};
use plotters::{
    chart::{ChartBuilder, ChartContext},
    coord::types::RangedCoordusize,
    prelude::{
        Cartesian2d, DrawingAreaErrorKind, DrawingBackend, IntoDrawingArea, Polygon, SVGBackend,
    },
    style::{RGBAColor, ShapeStyle, TRANSPARENT, TextStyle, full_palette::GREY_200},
};
use resvg::{tiny_skia, usvg};

use crate::splashes::splashlist::SplashList;

pub fn distribution_png_bytes(splashes: &SplashList) -> Result<Vec<u8>> {
    let svg = distribution_chart_svg(splashes)?;
    // NOTE: Charts are initially created using the `plotters` SVG backend, before being rendered
    // using `resvg` and encoded as a PNG to be sent on Discord. The reason for the SVG 'detour' is
    // that the Bitmap backend doesn't support transparency. (This won't be used often enough to
    // consider switching libraries at the moment)
    let png_buffer = render_svg(&svg)?;

    Ok(png_buffer)
}

fn distribution_chart_svg(splashes: &SplashList) -> Result<String> {
    const FONT_NAME: &str = "Noto Sans";
    const CHART_SIZE: (u32, u32) = (1600, 800);
    const LAYER_COLORS: [RGBAColor; 4] = [
        RGBAColor(221, 46, 68, 0.8),   // top 1
        RGBAColor(120, 177, 89, 0.8),  // top 2
        RGBAColor(85, 172, 238, 0.8),  // top 3
        RGBAColor(255, 255, 255, 0.8), // rest
    ];

    let per_day = splashes.split_days_top_3();
    let max_daily = per_day.iter().map(|d| d.iter().sum()).max().unwrap_or(0);
    // round up to nearest 10
    let chart_max = max_daily.div_ceil(10) * 10;

    let mut svg = String::new();

    {
        let root = SVGBackend::with_string(&mut svg, CHART_SIZE).into_drawing_area();

        let mut chart = ChartBuilder::on(&root)
            .margin_top(20)
            .margin_right(20)
            .x_label_area_size(100)
            .y_label_area_size(120)
            .build_cartesian_2d(1..splashes.bingo_days(), 0..chart_max as usize)?;

        chart
            .configure_mesh()
            .x_desc("Day")
            .y_desc("Splashes")
            .axis_desc_style(TextStyle::from((FONT_NAME, 60)).color(&GREY_200))
            .label_style(TextStyle::from((FONT_NAME, 40)).color(&GREY_200))
            .axis_style(GREY_200)
            .bold_line_style(GREY_200)
            .light_line_style(TRANSPARENT)
            .draw()?;

        let mut stacked_chart = StackedAreaChartContent::new();

        let chart_data = splashes.split_days_top_3();

        for layer_index in 0..4 {
            let points: Vec<_> = chart_data
                .iter()
                .enumerate()
                .map(|(i, day)| (i + 1, day[layer_index] as usize))
                .collect();
            stacked_chart.add_layer_relative(points, &LAYER_COLORS[layer_index]);
        }

        stacked_chart.draw(&mut chart)?;

        root.present()?;
    }

    Ok(svg)
}

// NOTE: custom implementation of a stacked area chart, as `plotters` only supports a constant
// baseline for any AreaSeries, meaning transparent colors would mix when drawing over other layers
struct StackedAreaChartContent<'a> {
    layers: Vec<(Vec<(usize, usize)>, &'a RGBAColor)>,
}

impl<'a> StackedAreaChartContent<'a> {
    pub fn new() -> Self {
        Self { layers: Vec::new() }
    }

    #[allow(dead_code)]
    pub fn add_layer_absolute(&mut self, points: Vec<(usize, usize)>, color: &'a RGBAColor) {
        self.layers.push((points, color));
    }

    pub fn add_layer_relative(&mut self, mut points: Vec<(usize, usize)>, color: &'a RGBAColor) {
        if !self.layers.is_empty() {
            let lower_points = &self.layers.last().unwrap().0;
            points
                .iter_mut()
                .enumerate()
                // offset upper bounds by last layer, which will be serving as lower bounds
                .for_each(|(i, (_, y))| *y += lower_points[i].1);
        }

        self.layers.push((points, color));
    }

    pub fn draw<DB: DrawingBackend>(
        &self,
        chart: &mut ChartContext<DB, Cartesian2d<RangedCoordusize, RangedCoordusize>>,
    ) -> Result<(), DrawingAreaErrorKind<DB::ErrorType>> {
        for (i, &(ref points, color)) in self.layers.iter().enumerate() {
            let mut polygon_points = points.to_vec();

            let lower_reversed = if i == 0 {
                // baseline as lower bounds
                (1..=polygon_points.len()).map(|i| (i, 0)).rev().collect()
            } else {
                // next lower layer as lower bounds
                let mut lower_points = self.layers[i - 1].0.to_vec();
                lower_points.reverse();
                lower_points
            };

            polygon_points.extend(lower_reversed);

            chart.draw_series(std::iter::once(Polygon::new(
                polygon_points,
                ShapeStyle::from(color).filled(),
            )))?;
        }
        Ok(())
    }
}

/// Renders SVG string, returns encoded PNG data
fn render_svg(svg: &str) -> Result<Vec<u8>> {
    let mut opt = usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();

    let tree = usvg::Tree::from_str(svg, &opt)?;
    let size = tree.size().to_int_size();

    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())
        .context("Failed to create pixmap with requested size")?;
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    let mut png_bytes: Vec<u8> = Vec::new();

    let img: ImageBuffer<Rgba<u8>, _> =
        ImageBuffer::from_raw(size.width(), size.height(), pixmap.data())
            .context("Failed to build image from rgba buffer")?;

    img.write_to(&mut std::io::Cursor::new(&mut png_bytes), ImageFormat::Png)?;

    Ok(png_bytes)
}
