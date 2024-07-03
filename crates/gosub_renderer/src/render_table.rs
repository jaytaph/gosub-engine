// const MAX_TABLE_COLS: u32 = 200;
// const MAX_TABLE_ROWS: u32 = 200;


use std::cmp;

/*
How to build up a table. Attempt 1.

- Figure out the maximum number of columns and rows.
- Generate cells of even width and height to fit in the given width.
- Use the minimum height for each cell.
- Absorb any colspan into the cell. So cell1 with colspan 2 will add
  height/width of cell2, and cell2 will not exist.
- Absorb any rowspan into the cell. So cell1 with rowspan 2 will add
  the HEIGHT of the cell below and the cell below will not exist.
- Add any padding to the cells cell, if any. Note that this can change the max-height of the row
  so make sure all cells in the row will have the same height.
- How to deal with spacing?
- If a table is bordered, make sure we add spacing for the border.
- ???
- Profit
 */


trait TableRender {
    fn new(height: usize, width: usize) -> impl TableRender;
    fn put_str(&mut self, x: usize, y: usize, content: &str);
    fn put_ch(&mut self, x: usize, y: usize, c: char);
    fn render(&self) -> String;
}

struct StringRenderer {
    width: usize,
    height: usize,
    buf: Vec<char>,
}

impl TableRender for StringRenderer {
    fn new(height: usize, width: usize) -> StringRenderer {
        Self{
            width: width,
            height: height,
            buf: vec!(' '; width * height),
        }
    }

    fn put_str(&mut self, x: usize, y: usize, content: &str) {
        for i in 0..content.len() {
            self.put_ch(x + i, y, content.chars().nth(i).unwrap());
        }
    }

    fn put_ch(&mut self, x: usize, y: usize, c: char) {
        let idx = y * self.width + x;
        self.buf[idx] = c;
    }

    fn render(&self) -> String {
        let mut output = String::with_capacity(self.buf.len() + self.height);
        for i in 0..self.height {
            let start = i * self.width;
            let end = start + self.width;
            output.push_str(&self.buf[start..end].iter().collect::<String>());
            output.push('\n');
        }

        output
    }
}


/// Table direction
#[derive(Debug)]
pub enum TableDir {
    LTR,            // Table is left-to-right
    RTL,            // Table is right-to-left
}

/// Horizontal alignment of a cell
#[derive(Debug)]
pub enum TableHalign {
    LEFT,           // Align text to the left
    RIGHT,          // Align text to the right
    CENTER,         // Align text to the center
}

// Vertical alignment of a cell
#[derive(Debug)]
pub enum TableValign {
    TOP,            // Align text to the top
    BOTTOM,         // Align text to the bottom
    CENTER,         // Align text to the center
}

#[allow(dead_code)]
#[derive(Debug)]
// Calculated variables based on the table
struct TableVars {
    /// width (in chars) of the table, including a border
    width: usize,
    /// (max) Number of columns per row in the table
    num_cols: usize,
    /// (max) Number of rows in the table
    num_rows: usize,
    /// Height of the table in chars
    height: usize,
    /// Each cell offset in the table
    cells: Vec<TableCellVars>,
}

#[allow(dead_code)]
#[derive(Debug)]
struct TableCellVars {
    offset_x: usize,      // Offset of the cell, or the border if bordered
    offset_y: usize,      //
    width: usize,         // Width (incl border)
    height: usize,        // Height (incl border)
    bordered: bool,     // Bordered or not
    content: String,    // Actual content
}

impl TableVars {
    fn compute(table: &Table, width: usize) -> Self {

        let mut max_cols = 1;
        for row in &table.head_rows {
            max_cols = cmp::max(max_cols, calc_cols_in_row(&row.cells));
        }
        for row in &table.body_rows {
            max_cols = cmp::max(max_cols, calc_cols_in_row(&row.cells));
        }
        for row in &table.footer_rows {
            max_cols = cmp::max(max_cols, calc_cols_in_row(&row.cells));
        }

        let max_rows = table.head_rows.len() + table.body_rows.len() + table.footer_rows.len();

        let width = cmp::max(3, width); // Minimum width of 3 chars
        let height;
        if table.bordered {
            height = max_rows * 2 + 1;      // One extra for the bottom border line
        } else {
            height = max_rows;
        }

        let mut tablevars = Self {
            num_cols: max_rows,
            num_rows: max_cols,
            width: width,
            height: height,
            cells: Vec::new(),
        };

        let cell_height;
        if table.bordered {
            cell_height = 2;
        } else {
            cell_height = 1;
        }

        // Start with a regular table where all cells are the same size
        let cell_width = (width as f32 / max_rows as f32) as usize;

        for row in 0..max_rows {
            for col in 0..max_cols {
                tablevars.cells.push(TableCellVars {
                    offset_x: (cell_width * col),
                    offset_y: (cell_height * row),
                    width: cell_width,
                    height: cell_height,
                    bordered: table.bordered,
                    content: String::new(),
                });
            }
        }

        dbg!(&tablevars);
        tablevars
    }
}


/// Calculate the number of columns in this row. Note that colspans are counted as well.
fn calc_cols_in_row(row: &Vec<TableCell>) -> usize {
    let mut num_cols_in_row = 0;

    for cell in row.iter() {
        if cell.colspan > 0 {
            num_cols_in_row += cell.colspan;
        } else {
            num_cols_in_row += 1;
        }
    }

    num_cols_in_row
}


#[allow(dead_code)]
pub struct TableCell {
    pub content: String,            // Content, will adhere to \n for line breaks
    pub wrapping: bool,             // Any text should either wrap to next line or increases cell width
    pub h_alignment: TableHalign,          // Horizontal alignment: (L)eft (R)ight (C)enter
    pub v_alignment: TableValign,          // Vertical alignment: (T)op (B)ottom (C)enter
    pub colspan: usize,               // This cell spans X cols
    pub rowspan: usize,               // This cell spans X rows
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
    cell_spacing: usize,          // Spacing between cells
    cell_padding: usize,          // Padding inside cells
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

    pub fn with_cell_spacing(mut self, cell_spacing: usize) -> Self {
        self.cell_spacing = cell_spacing;
        self
    }

    pub fn with_cell_padding(mut self, cell_padding: usize) -> Self {
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
        let width = cmp::max(3, width);
        let tv = TableVars::compute(self, width);

        let output = StringRenderer::new(tv.height, tv.width);
        self.render_ruler(&output, tv.width);
        self.render_caption(&output, &tv);

        if self.bordered {
            self.render_bordered(&output, &tv);
        } else {
            self.render_unbordered(&output, &tv);
        }

        output.render()
    }

    fn render_caption(&self, _renderer: &impl TableRender, vars: &TableVars) -> String {
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

    fn render_bordered(&self, _renderer: &impl TableRender, _vars: &TableVars) -> String {
        let output = String::new();
        // output.push_str(self.render_border_top(width).as_str());
        // output.push_str(self.render_border_row(width, &self.head_rows).as_str());
        // output.push_str(self.render_border_row(width, &self.body_rows).as_str());
        // output.push_str(self.render_border_row(width, &self.footer_rows).as_str());
        // output.push_str(self.render_border_bottom(width).as_str());
        output
    }

    fn render_unbordered(&self, _renderer: &impl TableRender, _vars: &TableVars) -> String {
        let output = String::new();
        // output.push_str(self.render_row(&self.head_rows).as_str());
        // output.push_str(self.render_row(&self.body_rows).as_str());
        // output.push_str(self.render_row(&self.footer_rows).as_str());
        output
    }

    fn render_ruler(&self, _renderer: &impl TableRender, width: usize) -> String {
        // Render a ruler for the table

        // 0         1         2         3         4         5         6
        // 0123456789012345678901234567890123456789012345678901234567890123456789

        let mut ruler_top = String::new();
        let mut ruler_bottom = String::new();

        for i in 0..(width / 10) {
            let number = format!("{:<width$}", i, width = 10);
            ruler_top.push_str(&number);
        }

        for i in 0..width {
            ruler_bottom.push_str(&(i % 10) .to_string());
        }

        format!("{}\n{}\n", ruler_top, ruler_bottom)
    }
}


/***

    <table>
        <tr>
            <td colspan=2>a</td>
            <td>b</td>
            <td>c</td>
        </tr>
        <tr>
            <td>d</td>
            <td>e</td>
            <td>f</td>
            <td rowspan=2>g</td>
        </tr>
        <tr>
            <td>h</td>
            <td>i</td>
            <td>j</td>
        </tr>
    </table>

***/



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stringrenderer() {
        let mut sr = StringRenderer::new(20, 20);
        for i in 0..20 {
            sr.put_ch(19, i, '.');
        }
        sr.put_str(3, 3, "Hello, world!");
        sr.put_str(0, 0, "A");
        sr.put_str(0, 19, "B");
        sr.put_str(19, 19, "C");
        sr.put_str(19, 0, "D");
        sr.put_ch( 9,  9, '1');
        sr.put_ch(10,  9, '2');
        sr.put_ch(11,  9, '3');
        sr.put_ch( 9, 10, '4');
        sr.put_ch(10, 10, 'X');
        sr.put_ch(11, 10, '5');
        sr.put_ch( 9, 11, '6');
        sr.put_ch(10, 11, '7');
        sr.put_ch(11, 11, '8');
        sr.put_str(18, 5, "WRAP");

        assert_eq!(sr.render(), r"A                  D
                   .
                   .
   Hello, world!   .
                   .
                  WR
AP                 .
                   .
                   .
         123       .
         4X5       .
         678       .
                   .
                   .
                   .
                   .
                   .
                   .
                   .
B                  C
");
    }
}