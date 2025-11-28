use crate::tui::app::App;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use ratatui_macros::{line, span};

const LABEL: Style = Style::new().add_modifier(Modifier::DIM);
const BOLD: Style = Style::new().add_modifier(Modifier::BOLD);

fn soc_bar(soc: f32, width: usize) -> String {
    let soc = soc.clamp(0.0, 100.0);
    let filled = ((soc / 100.0) * width as f32) as usize;
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

fn min_max(values: &[f32]) -> Option<(f32, f32)> {
    if values.is_empty() {
        return None;
    }
    let min = values.iter().cloned().fold(f32::MAX, f32::min);
    let max = values.iter().cloned().fold(f32::MIN, f32::max);
    Some((min, max))
}

fn color_current(amps: f32) -> Color {
    if amps >= 0.0 {
        Color::Green
    } else {
        Color::Yellow
    }
}

fn color_soc(soc: f32) -> Color {
    if soc >= 50.0 {
        Color::Green
    } else if soc >= 20.0 {
        Color::Yellow
    } else {
        Color::Red
    }
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Min(14),
            Constraint::Length(1),
        ])
        .split(frame.area());

    draw_rollup(frame, app, chunks[0]);
    draw_main_area(frame, app, chunks[1]);
    draw_status_bar(frame, app, chunks[2]);
}

fn draw_rollup(frame: &mut Frame, app: &App, area: Rect) {
    let rollup = app.rollup();

    let temp_str = match (rollup.min_temperature, rollup.max_temperature) {
        (Some(min), Some(max)) => format!("{:.1}-{:.1}C", min, max),
        _ => "N/A".to_string(),
    };

    let sign = if rollup.total_current >= 0.0 { "+" } else { "" };
    let soc = rollup.average_soc;
    let bar = soc_bar(soc, 40);

    let lines = vec![
        line![
            span!(LABEL; "Current: "),
            span!(Style::default().fg(color_current(rollup.total_current)); format!("{sign}{:.1}A", rollup.total_current)),
            "    ",
            span!(LABEL; "Capacity: "),
            format!(
                "{:.0}/{:.0}Ah",
                rollup.total_remaining_ah, rollup.total_capacity_ah
            ),
            "    ",
            span!(LABEL; "Temp: "),
            span!(Style::default().fg(Color::Cyan); temp_str),
        ],
        line![],
        line![
            span!(LABEL; "SOC: "),
            span!(Style::default().fg(color_soc(soc)); format!("{:5.1}% ", soc)),
            span!(Style::default().fg(color_soc(soc)); bar),
        ],
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(
            " Roll Up ({}/{}) ",
            rollup.responding_count, rollup.battery_count
        ))
        .title_style(BOLD);

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_main_area(frame: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(18), Constraint::Min(40)])
        .split(area);

    draw_battery_list(frame, app, chunks[0]);
    draw_battery_detail(frame, app, chunks[1]);
}

fn draw_battery_list(frame: &mut Frame, app: &mut App, area: Rect) {
    let items: Vec<ListItem> = app
        .batteries
        .iter()
        .map(|(addr, info)| {
            let voltage_str = info
                .as_ref()
                .map(|b| format!("{:.1}V", b.module_voltage))
                .unwrap_or_else(|| "---".to_string());

            let content = format!("0x{:02X} {}", addr, voltage_str);

            let style = if info.is_some() {
                Style::default()
            } else {
                LABEL
            };

            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Batteries ")
                .title_style(BOLD),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, &mut app.list_state);
}

fn draw_battery_detail(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Battery Details ")
        .title_style(BOLD);

    let Some(battery) = app.selected_battery() else {
        let addr = app
            .batteries
            .get(app.selected())
            .map(|(a, _)| *a)
            .unwrap_or(0);
        frame.render_widget(
            Paragraph::new(format!("No data for 0x{:02X}", addr)).block(block),
            area,
        );
        return;
    };

    let sign = if battery.current >= 0.0 { "+" } else { "" };
    let soc = battery.soc_percent;
    let bar = soc_bar(soc, 40);

    let mut lines: Vec<Line> = vec![
        line![
            span!(BOLD; &battery.model),
            "  ",
            span!(LABEL; "SN:"),
            Span::raw(&battery.serial),
            "  ",
            Span::raw(&battery.software_version),
        ],
        line![],
        line![
            span!(LABEL; "Voltage: "),
            span!(Style::default().fg(Color::Cyan); format!("{:.2}V", battery.module_voltage)),
            "    ",
            span!(LABEL; "Current: "),
            span!(Style::default().fg(color_current(battery.current)); format!("{sign}{:.2}A", battery.current)),
            "    ",
            span!(LABEL; "Cycles: "),
            format!("{}", battery.cycle_count),
        ],
        line![
            span!(LABEL; "Capacity: "),
            format!(
                "{:.1}/{:.1}Ah",
                battery.remaining_capacity, battery.total_capacity
            ),
        ],
        line![],
        line![
            span!(LABEL; "SOC: "),
            span!(Style::default().fg(color_soc(soc)); format!("{:5.1}% ", soc)),
            span!(Style::default().fg(color_soc(soc)); bar),
        ],
        line![],
    ];

    // Temperatures
    if let Some((min_t, max_t)) = min_max(&battery.cell_temperatures) {
        lines.push(line![
            span!(LABEL; "Temp: "),
            span!(Style::default().fg(Color::Cyan); format!("{:.1}-{:.1}C", min_t, max_t)),
            span!(LABEL; format!(" ({} sensors)", battery.cell_temperatures.len())),
        ]);
    } else {
        lines.push(line![span!(LABEL; "Temp: "), "(no data)"]);
    }

    lines.push(line![]);

    // Cell voltages
    if let Some((min_v, max_v)) = min_max(&battery.cell_voltages) {
        let delta = max_v - min_v;
        lines.push(line![
            span!(LABEL; format!("Cells[{}]: ", battery.cell_voltages.len())),
            span!(Style::default().fg(Color::Red); format!("{:.3}", min_v)),
            "-",
            span!(Style::default().fg(Color::Green); format!("{:.3}V", max_v)),
            span!(LABEL; format!(" Δ{:3.0}mV", delta * 1000.0)),
        ]);

        for (i, chunk) in battery.cell_voltages.chunks(4).enumerate() {
            let row_start = i * 4 + 1;
            let mut spans: Vec<Span> = vec![span!(LABEL; format!(" {:>2}: ", row_start))];
            for &v in chunk {
                let style = if delta > 0.005 && (v - min_v).abs() < 0.001 {
                    Style::default().fg(Color::Red)
                } else if delta > 0.005 && (v - max_v).abs() < 0.001 {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default()
                };
                spans.push(span!(style; format!("{:.3}  ", v)));
            }
            lines.push(Line::from(spans));
        }
    } else {
        lines.push(line![
            span!(LABEL; format!("Cells[{}]: ", battery.cell_count)),
            "(no voltage data)",
        ]);
    }

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let last_update = app
        .last_update
        .map(|t| {
            let secs = t.elapsed().as_secs();
            if secs < 60 {
                format!("{}s", secs)
            } else {
                format!("{}m", secs / 60)
            }
        })
        .unwrap_or_else(|| "-".to_string());

    let status = if app.refreshing {
        span!(Style::default().fg(Color::Yellow); "Refreshing")
    } else if app.error.is_some() {
        span!(Style::default().fg(Color::Red); "Error")
    } else {
        span!(Style::default().fg(Color::Green); "OK")
    };

    let line = line![
        span!(LABEL; " q"),
        ":quit ",
        span!(LABEL; "jk"),
        ":sel ",
        span!(LABEL; "r"),
        ":refresh | ",
        status,
        format!(" | {}", last_update),
    ];

    frame.render_widget(Paragraph::new(line), area);
}
