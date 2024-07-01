// const MAX_TABLE_COLS: u32 = 200;
// const MAX_TABLE_ROWS: u32 = 200;


use std::cmp;


enum TableDir {
    LTR,
    RTL,
}

enum TableHalign {
    LEFT,
    RIGHT,
    CENTER,
}

enum TableValign {
    TOP,
    BOTTOM,
    CENTER,
}

// Calculated variables based on the table
struct TableVars {
    // width (in chars) of the table, including a border
    width: usize,
    // (max) Number of columns per row in the table
    num_cols: u32,
    // (max) Number of rows in the table
    num_rows: u32,
    // Height of the table in chars
    height: u32,
    // Offsets for each column in the table
    offsets: Vec<Vec<u32>>,
}

impl TableVars {
    fn compute(table: &Table, width: usize) -> Self {
        let width = cmp::max(3, width); // Minimum width of 3 chars
        let height = 2; // Minimum height of 2 chars

        Self {
            num_cols: Self::compute_max_cols(table),
            num_rows: Self::compute_max_rows(table),
            width: Self::Compute_width(table),
            height: Self::compute_height(table),
            offsets: Self::compute_offsets(table),
        }
    }
}


#[allow(dead_code)]
pub struct TableCell {
    pub content: String,            // Content, will adhere to \n for line breaks
    pub wrapping: bool,             // Any text should either wrap to next line or increases cell width
    pub h_alignment: TableHalign,          // Horizontal alignment: (L)eft (R)ight (C)enter
    pub v_alignment: TableValign,          // Vertical alignment: (T)op (B)ottom (C)enter
    pub colspan: u32,               // This cell spans X cols
    pub rowspan: u32,               // This cell spans X rows
}

#[allow(dead_code)]
pub struct TableRow {
    pub cells: Vec<TableCell>,      // Number of cells in the row. Does not have to be equal for all rows
}

#[allow(dead_code)]
pub struct Table {
    summary: String,            // Screen reader summary
    caption: String,            // Caption of the table, if set it will be displayed
    direction: TableDir,          // LTR or RTL direction
    head_rows: Vec<TableRow>,   // <THEAD> rows
    body_rows: Vec<TableRow>,   // <TBODY> (default) rows
    footer_rows: Vec<TableRow>, // <TFOOT> rows
    bordered: bool,             // Cells are bordered or not
    cell_spacing: u32,          // Spacing between cells
    cell_padding: u32,          // Padding inside cells
}

#[allow(dead_code)]
impl Table {
    pub fn new() -> Self {
        Self {
            summary: String::new(),
            caption: String::new(),
            direction: TableDir::LTR,
            head_rows: Vec::new(),
            body_rows: Vec::new(),
            footer_rows: Vec::new(),
            bordered: false,
            cell_spacing: 0,
            cell_padding: 0,
        }
    }

    pub fn with_summary(mut self, summary: &str) -> Self {
        self.summary = summary.into();
        self
    }

    pub fn with_caption(mut self, caption: &str) -> Self {
        self.caption = caption.into();
        self
    }

    pub fn with_direction(mut self, direction: TableDir) -> Self {
        self.direction = direction;
        self
    }

    pub fn with_head_rows(mut self, head_rows: Vec<TableRow>) -> Self {
        self.head_rows = head_rows;
        self
    }

    pub fn with_body_rows(mut self, body_rows: Vec<TableRow>) -> Self {
        self.body_rows = body_rows;
        self
    }

    pub fn with_footer_rows(mut self, footer_rows: Vec<TableRow>) -> Self {
        self.footer_rows = footer_rows;
        self
    }

    pub fn with_bordered(mut self, bordered: bool) -> Self {
        self.bordered = bordered;
        self
    }

    pub fn with_cell_spacing(mut self, cell_spacing: u32) -> Self {
        self.cell_spacing = cell_spacing;
        self
    }

    pub fn with_cell_padding(mut self, cell_padding: u32) -> Self {
        self.cell_padding = cell_padding;
        self
    }

    pub fn add_head_row(&mut self, row: TableRow) {
        self.head_rows.push(row);
    }

    pub fn add_body_row(&mut self, row: TableRow) {
        self.body_rows.push(row);
    }

    pub fn add_footer_row(&mut self, row: TableRow) {
        self.footer_rows.push(row);
    }

    pub fn render(&self, width: usize) -> String {
        let mut width = cmp::max(3, width);
        let tv = TableVars::compute(self, width);


        let mut output = String::new();

        output.push_str("+");
        output.push_str("-".repeat(width-2).as_str());
        output.push_str("+");
        output.push('\n');

        output.push_str(self.render_caption(t).as_str());

        if self.bordered {
            output.push_str(self.render_bordered(t).as_str())
        } else {
            output.push_str(self.render_unbordered(t).as_str())
        }

        output
    }

    fn render_caption(&self, vars: TableVars) -> String {
        if self.caption.is_empty() {
            return String::new();
        }

        let mut caption = self.caption.clone();

        // Cap the caption to the width
        if self.caption.len() > vars.width {
            caption = format!("{}...\n", caption[..vars.width].to_string());
        }

        // Center caption if there is room to place it
        if caption.len() <= vars.width {
            let total_padding = vars.width - caption.len();
            let left_pad = total_padding / 2;
            let right_pad = total_padding - left_pad;

            return format!("{}{}{}\n", " ".repeat(left_pad), caption, " ".repeat(right_pad));
        }

        return "".into();
    }

    fn render_bordered(&self, vars: TableVars) -> String {
        let mut output = String::new();
        // output.push_str(self.render_border_top(width).as_str());
        // output.push_str(self.render_border_row(width, &self.head_rows).as_str());
        // output.push_str(self.render_border_row(width, &self.body_rows).as_str());
        // output.push_str(self.render_border_row(width, &self.footer_rows).as_str());
        // output.push_str(self.render_border_bottom(width).as_str());
        output
    }

    fn render_unbordered(&self, vars: TableVars) -> String {
        let mut output = String::new();
        // output.push_str(self.render_row(&self.head_rows).as_str());
        // output.push_str(self.render_row(&self.body_rows).as_str());
        // output.push_str(self.render_row(&self.footer_rows).as_str());
        output
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_1() {
        let mut t = Table::new()
            .with_summary("This is the summary of the table")
            .with_caption("This is the caption of the table")
            .with_bordered(true)
            .with_cell_spacing(1)
            .with_cell_padding(1)
            .with_width(100)
            .with_height(100);

        t.add_head_row(TableRow {
            cells: vec![
                TableCell {
                    content: "Head 1".to_string(),
                    colspan: 1,
                    rowspan: 1,
                },
                TableCell {
                    content: "Head 2".to_string(),
                    colspan: 1,
                    rowspan: 1,
                },
            ]
        });

        t.add_body_row(TableRow {
            cells: vec![
                TableCell {
                    content: "Body 1".to_string(),
                    colspan: 1,
                    rowspan: 1,
                },
                TableCell {
                    content: "Body 2".to_string(),
                    colspan: 1,
                    rowspan: 1,
                },
            ]
        });

        t.add_footer_row(TableRow {
            cells: vec![
                TableCell {
                    content: "Footer 1".to_string(),
                    colspan: 1,
                    rowspan: 1,
                },
                TableCell {
                    content: "Footer 2".to_string(),
                    colspan: 1,
                    rowspan: 1,
                },
            ]
        });

        println!("{}", t.render(80));
    }
}