use std::time::{Duration, Instant};

use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Gauge, Paragraph, Row, Table},
    DefaultTerminal, Frame,
};

use crate::{
    collector::Collector,
    fmt::{format_uptime, human_bytes, human_rate},
    model::Snapshot,
};

const MIN_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, PartialEq)]
enum SortKey {
    Cpu,
    Mem,
    Name,
}

struct App {
    collector: Collector,
    snapshot: Snapshot,
    sort: SortKey,
    paused: bool,
    /// Divide process CPU% by the core count so 100% means the whole machine.
    normalize_cpu: bool,
    interval: Duration,
    last_refresh: Instant,
}

pub fn run(interval: Duration, normalize_cpu: bool) -> anyhow::Result<()> {
    let mut collector = Collector::new();
    let snapshot = collector.collect();
    let mut app = App {
        collector,
        snapshot,
        sort: SortKey::Cpu,
        paused: false,
        normalize_cpu,
        interval,
        last_refresh: Instant::now(),
    };

    // `init` also installs a panic hook that restores the terminal.
    let mut terminal = ratatui::init();
    let result = app.event_loop(&mut terminal);
    ratatui::restore();
    result
}

impl App {
    fn event_loop(&mut self, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
        loop {
            if !self.paused && self.last_refresh.elapsed() >= self.interval {
                self.snapshot = self.collector.collect();
                self.last_refresh = Instant::now();
            }
            terminal.draw(|f| self.draw(f))?;

            if event::poll(Duration::from_millis(250))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('c') => self.sort = SortKey::Cpu,
                    KeyCode::Char('m') => self.sort = SortKey::Mem,
                    KeyCode::Char('p') => self.sort = SortKey::Name,
                    KeyCode::Char(' ') => self.paused = !self.paused,
                    KeyCode::Char('n') => self.normalize_cpu = !self.normalize_cpu,
                    KeyCode::Char('+') => self.interval += Duration::from_millis(500),
                    KeyCode::Char('-') => {
                        self.interval = self
                            .interval
                            .saturating_sub(Duration::from_millis(500))
                            .max(MIN_INTERVAL)
                    }
                    _ => {}
                }
            }
        }
    }

    fn draw(&self, f: &mut Frame) {
        let [header, gauges, middle, procs, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(6),
            Constraint::Percentage(45),
            Constraint::Length(1),
        ])
        .areas(f.area());

        self.draw_header(f, header);
        self.draw_gauges(f, gauges);
        let [left, right] =
            Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)])
                .areas(middle);
        self.draw_cores(f, left);
        self.draw_net_disks(f, right);
        self.draw_processes(f, procs);
        self.draw_footer(f, footer);
    }

    fn draw_header(&self, f: &mut Frame, area: Rect) {
        let h = &self.snapshot.host;
        let load = match h.load_avg {
            Some([a, b, c]) => format!("{a:.2} {b:.2} {c:.2}"),
            None => "n/a".into(),
        };
        let line = Line::from(vec![
            Span::from(h.hostname.as_str()).bold().cyan(),
            Span::raw(format!("  {}", h.os)),
            Span::raw(format!("  up {}", format_uptime(h.uptime))),
            Span::raw(format!("  load {load}")),
        ]);
        f.render_widget(Paragraph::new(line), area);
    }

    fn draw_gauges(&self, f: &mut Frame, area: Rect) {
        let s = &self.snapshot;
        let areas = Layout::horizontal([Constraint::Ratio(1, 3); 3]).split(area);
        let items = [
            ("CPU", s.cpu.total, format!("{:.1}%", s.cpu.total)),
            ("MEM", s.mem.percent, usage_label(s.mem.percent, s.mem.used, s.mem.total)),
            ("SWAP", s.swap.percent, usage_label(s.swap.percent, s.swap.used, s.swap.total)),
        ];
        for ((title, pct, label), area) in items.into_iter().zip(areas.iter()) {
            let gauge = Gauge::default()
                .block(Block::bordered().title(title))
                .gauge_style(Style::new().fg(level_color(pct)))
                .ratio((pct as f64 / 100.0).clamp(0.0, 1.0))
                .label(label);
            f.render_widget(gauge, *area);
        }
    }

    fn draw_cores(&self, f: &mut Frame, area: Rect) {
        let block = Block::bordered().title("Cores");
        let inner = block.inner(area);
        f.render_widget(block, area);

        let cores = &self.snapshot.cpu.cores;
        let rows = inner.height.max(1) as usize;
        let cols = cores.len().div_ceil(rows).max(1);
        let col_areas = Layout::horizontal(vec![Constraint::Ratio(1, cols as u32); cols]).split(inner);

        for (col, (chunk, col_area)) in cores.chunks(rows).zip(col_areas.iter()).enumerate() {
            let bar_width = (col_area.width as usize).saturating_sub(12).max(1);
            let lines: Vec<Line> = chunk
                .iter()
                .enumerate()
                .map(|(i, &pct)| {
                    let idx = col * rows + i;
                    let filled = ((pct / 100.0) * bar_width as f32).round() as usize;
                    Line::from(vec![
                        Span::raw(format!("{idx:>3} ")),
                        Span::styled("|".repeat(filled.min(bar_width)), Style::new().fg(level_color(pct))),
                        Span::raw(" ".repeat(bar_width - filled.min(bar_width))),
                        Span::raw(format!("{pct:>5.1}%")),
                    ])
                })
                .collect();
            f.render_widget(Paragraph::new(lines), *col_area);
        }
    }

    fn draw_net_disks(&self, f: &mut Frame, area: Rect) {
        let s = &self.snapshot;
        let [net_area, disk_area] =
            Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(area);

        let net_rows = s.nets.iter().map(|n| {
            Row::new(vec![n.name.clone(), human_rate(n.rx_rate), human_rate(n.tx_rate)])
        });
        let net = Table::new(net_rows, [Constraint::Fill(1), Constraint::Length(11), Constraint::Length(11)])
            .header(header_row(["Interface", "Rx", "Tx"]))
            .block(Block::bordered().title("Network"));
        f.render_widget(net, net_area);

        let disk_rows = s.disks.iter().map(|d| {
            Row::new(vec![
                Span::raw(d.mount.clone()),
                Span::raw(human_bytes(d.used)),
                Span::raw(human_bytes(d.total)),
                Span::styled(format!("{:.1}%", d.percent), Style::new().fg(level_color(d.percent))),
            ])
        });
        let disks = Table::new(
            disk_rows,
            [Constraint::Fill(1), Constraint::Length(8), Constraint::Length(8), Constraint::Length(7)],
        )
        .header(header_row(["Mount", "Used", "Total", "Use%"]))
        .block(Block::bordered().title("Disks"));
        f.render_widget(disks, disk_area);
    }

    fn draw_processes(&self, f: &mut Frame, area: Rect) {
        let mut procs: Vec<_> = self.snapshot.processes.iter().collect();
        match self.sort {
            SortKey::Cpu => procs.sort_by(|a, b| b.cpu.total_cmp(&a.cpu)),
            SortKey::Mem => procs.sort_by_key(|p| std::cmp::Reverse(p.mem)),
            SortKey::Name => procs.sort_by_key(|p| p.name.to_lowercase()),
        }

        let divisor = if self.normalize_cpu {
            self.snapshot.cpu.cores.len().max(1) as f32
        } else {
            1.0
        };
        let rows = procs.iter().take(area.height as usize).map(|p| {
            let cpu = p.cpu / divisor;
            Row::new(vec![
                Span::raw(p.pid.to_string()),
                Span::styled(format!("{cpu:.1}"), Style::new().fg(level_color(cpu))),
                Span::raw(format!("{:.1}", p.mem_percent)),
                Span::raw(human_bytes(p.mem)),
                Span::raw(p.name.clone()),
            ])
        });

        let mark = |key: SortKey, label: &'static str| {
            if self.sort == key { format!("{label}▼") } else { label.to_string() }
        };
        let header = header_row([
            "PID".to_string(),
            mark(SortKey::Cpu, "CPU%"),
            mark(SortKey::Mem, "MEM%"),
            "RES".to_string(),
            mark(SortKey::Name, "Name"),
        ]);
        let table = Table::new(
            rows,
            [
                Constraint::Length(8),
                Constraint::Length(7),
                Constraint::Length(6),
                Constraint::Length(8),
                Constraint::Fill(1),
            ],
        )
        .header(header)
        .block(Block::bordered().title(format!("Processes ({})", procs.len())));
        f.render_widget(table, area);
    }

    fn draw_footer(&self, f: &mut Frame, area: Rect) {
        let state = if self.paused { " PAUSED ".black().on_yellow() } else { Span::raw("") };
        let line = Line::from(vec![
            state,
            Span::raw(format!(
                " q quit  c/m/p sort  space pause  n cpu% ({})  +/- interval ({:.1}s)",
                if self.normalize_cpu { "total" } else { "per core" },
                self.interval.as_secs_f32()
            ))
            .dark_gray(),
        ]);
        f.render_widget(Paragraph::new(line), area);
    }
}

fn header_row<S: Into<String>, const N: usize>(cells: [S; N]) -> Row<'static> {
    Row::new(cells.into_iter().map(Into::into).collect::<Vec<String>>())
        .style(Style::new().add_modifier(Modifier::BOLD))
}

fn usage_label(pct: f32, used: u64, total: u64) -> String {
    format!("{pct:.1}%  {} / {}", human_bytes(used), human_bytes(total))
}

/// glances thresholds: <50% ok, <80% warning, otherwise critical.
fn level_color(pct: f32) -> Color {
    match pct {
        p if p < 50.0 => Color::Green,
        p if p < 80.0 => Color::Yellow,
        _ => Color::Red,
    }
}
