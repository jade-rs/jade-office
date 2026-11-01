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

/// The dialog's print options, each on or off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrintOption {
    CenterHorizontally,
    CenterVertically,
    GridLines,
}

impl PrintOption {
    pub const ALL: [PrintOption; 3] = [
        PrintOption::CenterHorizontally,
        PrintOption::CenterVertically,
        PrintOption::GridLines,
    ];

    /// The i18n key of its name.
    pub fn label(self) -> &'static str {
        match self {
            PrintOption::CenterHorizontally => "page_setup.center.horizontally",
            PrintOption::CenterVertically => "page_setup.center.vertically",
            PrintOption::GridLines => "page_setup.gridlines",
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
        }
    }

    pub fn toggle_print_option(&mut self, option: PrintOption) {
        let on = match option {
            PrintOption::CenterHorizontally => &mut self.setup.center_horizontally,
            PrintOption::CenterVertically => &mut self.setup.center_vertically,
            PrintOption::GridLines => &mut self.setup.print_grid_lines,
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
