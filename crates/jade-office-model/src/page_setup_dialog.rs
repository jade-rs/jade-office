//! Excel's Page Setup dialog for a sheet: orientation, paper, scaling —
//! a percentage, or fitting to pages — the Normal, Wide and Narrow
//! margins, and centring on the page and printing the gridlines, written
//! to the sheet's `pageSetup`, `pageMargins` and `printOptions` as one
//! undo step.

use jade_xlsx_reader::{PageMargins, PageOrientation, PageSetup};

use crate::sheet::Workbook;
use crate::Backend;

/// The papers the dialog offers, as Excel's `paperSize` codes and the i18n
/// keys of their names.
pub const PAPERS: [(u32, &str); 6] = [
    (9, "page_setup.paper.a4"),
    (8, "page_setup.paper.a3"),
    (13, "page_setup.paper.b5"),
    (12, "page_setup.paper.b4"),
    (1, "page_setup.paper.letter"),
    (5, "page_setup.paper.legal"),
];

/// Excel's margin presets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarginPreset {
    Normal,
    Wide,
    Narrow,
}

impl MarginPreset {
    pub const ALL: [MarginPreset; 3] = [
        MarginPreset::Normal,
        MarginPreset::Wide,
        MarginPreset::Narrow,
    ];

    /// The i18n key of its name.
    pub fn label(self) -> &'static str {
        match self {
            MarginPreset::Normal => "page_setup.margins.normal",
            MarginPreset::Wide => "page_setup.margins.wide",
            MarginPreset::Narrow => "page_setup.margins.narrow",
        }
    }

    /// Its margins, in inches, as Excel's Margins gallery has them.
    pub fn margins(self) -> PageMargins {
        match self {
            MarginPreset::Normal => PageMargins::default(),
            MarginPreset::Wide => PageMargins {
                left: 1.0,
                right: 1.0,
                top: 1.0,
                bottom: 1.0,
                header: 0.5,
                footer: 0.5,
            },
            MarginPreset::Narrow => PageMargins {
                left: 0.25,
                right: 0.25,
                top: 0.75,
                bottom: 0.75,
                header: 0.3,
                footer: 0.3,
            },
        }
    }
}

/// The ways the dialog fits a sheet to pages: wide by tall, 0 for as many
/// as it takes, with the i18n keys of their names.
pub const FITS: [((u32, u32), &str); 3] = [
    ((1, 1), "page_setup.fit.page"),
    ((1, 0), "page_setup.fit.columns"),
    ((0, 1), "page_setup.fit.rows"),
];

/// A page margin the dialog steps on its own, as Excel's Margins tab
/// has a box for each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarginSide {
    Top,
    Bottom,
    Left,
    Right,
    Header,
    Footer,
}

impl MarginSide {
    pub const ALL: [MarginSide; 6] = [
        MarginSide::Top,
        MarginSide::Bottom,
        MarginSide::Left,
        MarginSide::Right,
        MarginSide::Header,
        MarginSide::Footer,
    ];

    /// The i18n key of its name.
    pub fn label(self) -> &'static str {
        match self {
            MarginSide::Top => "page_setup.margin.top",
            MarginSide::Bottom => "page_setup.margin.bottom",
            MarginSide::Left => "page_setup.margin.left",
            MarginSide::Right => "page_setup.margin.right",
            MarginSide::Header => "page_setup.margin.header",
            MarginSide::Footer => "page_setup.margin.footer",
        }
    }

    fn of(self, margins: &mut PageMargins) -> &mut f64 {
        match self {
            MarginSide::Top => &mut margins.top,
            MarginSide::Bottom => &mut margins.bottom,
            MarginSide::Left => &mut margins.left,
            MarginSide::Right => &mut margins.right,
            MarginSide::Header => &mut margins.header,
            MarginSide::Footer => &mut margins.footer,
        }
    }
}

/// How far one press of a margin's − or + moves it, in inches.
pub const MARGIN_STEP: f64 = 0.1;

/// The dialog's print options, each on or off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrintOption {
    CenterHorizontally,
    CenterVertically,
    GridLines,
    Headings,
}

impl PrintOption {
    pub const ALL: [PrintOption; 4] = [
        PrintOption::CenterHorizontally,
        PrintOption::CenterVertically,
        PrintOption::GridLines,
        PrintOption::Headings,
    ];

    /// The i18n key of its name.
    pub fn label(self) -> &'static str {
        match self {
            PrintOption::CenterHorizontally => "page_setup.center.horizontally",
            PrintOption::CenterVertically => "page_setup.center.vertically",
            PrintOption::GridLines => "page_setup.gridlines",
            PrintOption::Headings => "page_setup.headings",
        }
    }
}

/// The dialog: the sheet's page setup as it is being changed.
#[derive(Clone, Debug, PartialEq)]
pub struct PageSetupDialog {
    pub setup: PageSetup,
}

impl PageSetupDialog {
    /// Opened on the active sheet's page setup.
    pub fn open<B: Backend>(book: &Workbook<B>) -> Self {
        Self {
            setup: book.page_setup().clone(),
        }
    }

    pub fn landscape(&self) -> bool {
        self.setup.orientation == Some(PageOrientation::Landscape)
    }

    pub fn set_landscape(&mut self, landscape: bool) {
        self.setup.orientation = Some(if landscape {
            PageOrientation::Landscape
        } else {
            PageOrientation::Portrait
        });
    }

    pub fn set_paper(&mut self, code: u32) {
        self.setup.paper_size = Some(code);
    }

    /// The preset the margins are, if they are one.
    pub fn margin_preset(&self) -> Option<MarginPreset> {
        let margins = self.setup.margins.unwrap_or_default();
        MarginPreset::ALL
            .into_iter()
            .find(|preset| preset.margins() == margins)
    }

    pub fn set_margins(&mut self, preset: MarginPreset) {
        self.setup.margins = Some(preset.margins());
    }

    /// One margin, in inches.
    pub fn margin(&self, side: MarginSide) -> f64 {
        *side.of(&mut self.setup.margins.unwrap_or_default())
    }

    /// One margin moved `steps` of [`MARGIN_STEP`] up or down, kept between
    /// 0 and 10 inches and to the hundredth, as Excel's boxes keep it.
    pub fn step_margin(&mut self, side: MarginSide, steps: i32) {
        let mut margins = self.setup.margins.unwrap_or_default();
        let value = side.of(&mut margins);
        *value =
            ((*value + MARGIN_STEP * f64::from(steps)).clamp(0.0, 10.0) * 100.0).round() / 100.0;
        self.setup.margins = Some(margins);
    }

    /// The percentage it prints at when not fitting to pages.
    pub fn scale(&self) -> u32 {
        self.setup.scale.unwrap_or(100)
    }

    /// Adjust to: the percentage `by` points up or down, between Excel's 10
    /// and 400, printing at a scale rather than fitting.
    pub fn step_scale(&mut self, by: i32) {
        let scale = (self.scale() as i32 + by).clamp(10, 400) as u32;
        self.setup.scale = Some(scale);
        self.setup.fit_to_page = false;
    }

    pub fn print_option(&self, option: PrintOption) -> bool {
        match option {
            PrintOption::CenterHorizontally => self.setup.center_horizontally,
            PrintOption::CenterVertically => self.setup.center_vertically,
            PrintOption::GridLines => self.setup.print_grid_lines,
            PrintOption::Headings => self.setup.print_headings,
        }
    }

    pub fn toggle_print_option(&mut self, option: PrintOption) {
        let on = match option {
            PrintOption::CenterHorizontally => &mut self.setup.center_horizontally,
            PrintOption::CenterVertically => &mut self.setup.center_vertically,
            PrintOption::GridLines => &mut self.setup.print_grid_lines,
            PrintOption::Headings => &mut self.setup.print_headings,
        };
        *on = !*on;
    }

    /// Fit to `wide` by `tall` pages.
    pub fn set_fit(&mut self, (wide, tall): (u32, u32)) {
        self.setup.fit_to_page = true;
        self.setup.fit_to_width = Some(wide);
        self.setup.fit_to_height = Some(tall);
    }
}
