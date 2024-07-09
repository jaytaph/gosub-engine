use std::cmp;
use std::ops::Add;

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
    fn new(width: usize, height: usize) -> StringRenderer {
        Self{
            width,
            height,
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

/// A border style
#[allow(dead_code)]
struct BorderStyle {
    pub top_left: char,
    pub top_right: char,
    pub bottom_left: char,
    pub bottom_right: char,
    pub horizontal: char,
    pub vertical: char,
    pub cross: char,
}

impl BorderStyle {
    fn single() -> Self {
        Self {
            top_left: '┌',
            top_right: '┐',
            bottom_left: '└',
            bottom_right: '┘',
            horizontal: '─',
            vertical: '│',
            cross: '┼',
        }
    }
    fn double() -> Self {
        Self {
            top_left: '╔',
            top_right: '╗',
            bottom_left: '╚',
            bottom_right: '╝',
            horizontal: '═',
            vertical: '║',
            cross: '╬',
        }
    }

    #[allow(dead_code)]
    fn heavy() -> Self {
        Self {
            top_left: '┏',
            top_right: '┓',
            bottom_left: '┗',
            bottom_right: '┛',
            horizontal: '━',
            vertical: '┃',
            cross: '╋',
        }
    }

    #[allow(dead_code)]
    fn light() -> Self {
        Self {
            top_left: '┍',
            top_right: '┑',
            bottom_left: '┕',
            bottom_right: '┙',
            horizontal: '─',
            vertical: '│',
            cross: '┼',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord {
    /// X position
    pub x: usize,
    /// Y position
    pub y: usize,
    /// Width
    pub width: usize,
    /// Height
    pub height: usize,
}

impl Coord {
    pub fn new(x: usize, y: usize, width: usize, height: usize) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

impl Add for Coord {
    type Output = Coord;

    /// Coordinates can be added onto each other
    fn add(self, rhs: Self) -> Self::Output {
        Coord {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            width: rhs.width,
            height: rhs.height,
        }
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

#[derive(Debug, PartialEq)]
pub enum TableCaptionPos {
    TOP,
    BOTTOM,
}

#[allow(dead_code)]
#[derive(Debug)]
// Calculated variables based on the table
struct TableVars {
    /// (max) Number of columns per row in the table
    num_cols: usize,
    /// (max) Number of rows in the table
    num_rows: usize,
    /// Each cell offset in the table
    cells: Vec<TableCellVars>,
    // Total rectangle of the table including all parts like rulers and captions
    box_coords: Coord,
    // Box coordinates of the ruler, if any
    ruler_coords: Option<Coord>,
    // Box coordinates of the caption, if any
    caption_coords: Option<Coord>,
    // Box coordinates of the table itself
    table_coords: Coord
}

#[allow(dead_code)]
#[derive(Debug)]
struct TableCellVars {
    coord: Coord,
    // Bordered or not
    bordered: bool,
    // Actual content
    content: String,
}

impl TableVars {
    fn compute(table: &Table, box_width: usize) -> Self {
        ////////////////
        // Step: Calculate the maximum number of columns we can find in the table (including colspans)
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

        ////////////////
        // Step: Calculate the maximum number of rows in the table
        let max_rows = cmp::max(table.head_rows.len(), cmp::max(table.body_rows.len(), table.footer_rows.len()));


        ////////////////
        // Step: Setup coordinates for the different elements of the table
        let mut box_coords = Coord::new(0, 0, cmp::max(3, box_width), 0);
        let mut table_coords = Coord::new(0, 0, cmp::max(3, box_width), 0);
        let mut caption_coords = None;
        let mut ruler_coords = None;


        ////////////////
        // Step: Create actual table. Make sure width/height of table_coords are set properly
        ////////////////

        // We have max_cols and max_rows. It needs to fit in width chars, so calculate what each row width
        // should be (including borders)
        let cell_widths = calc_cell_width(box_width, max_cols);
        dbg!(&cell_widths);

        let cell_height;
        let cell_offset;
        if table.bordered {
            cell_height = 3;        // Each cell is 3 lines high
            cell_offset = 2;        // But start at offset 2, so they will overlap with the top of the previous cell

            // Increase height of table and box with the number of rows, including the borders and the fact that borders overlap
            table_coords.height += 2 * max_rows + 1;
            box_coords.height += 2 * max_rows + 1 + 1;

        } else {
            cell_height = 1;
            cell_offset = 1;

            // Increase height of table and box with the number of rows
            table_coords.height += max_rows;
            box_coords.height += max_rows;
        }

        // Start with a regular table where all cells are the same size
        let cell_width = (box_width as f32 / max_rows as f32) as usize;
        let mut cells = Vec::new();

        for row in 0..max_rows {
            let mut cur_x = 0;
            for col in 0..max_cols {
                cells.push(TableCellVars {
                    coord: Coord {
                        x: cur_x,
                        y: cell_offset * row,
                        width: cell_width,
                        height: cell_height,
                    },
                    bordered: table.bordered,
                    content: "Xcell contentX".into(),
                });
                cur_x += cell_widths[col] - 1;
            }
        }

        ////////////////
        // Step: Add additional ruler and caption

        if table.ruler {
            ruler_coords = Some(Coord::new(0, 0, box_coords.width, 2));
            // Move up the table and increase height
            table_coords.y += 2;
            box_coords.height += 2;
        }

        if !table.caption.is_empty() && table.caption_pos == TableCaptionPos::TOP {
            // Caption is at the top of the table, move table and increase height
            caption_coords = Some(Coord::new(0, 0, box_coords.width, 1));
            table_coords.y += 1;
            box_coords.height += 1;
        }

        if table.caption_pos == TableCaptionPos::BOTTOM {
            // Caption is at the bottom, so only increase height
            caption_coords = Some(Coord::new(0, box_coords.height, box_coords.width, 1));
            box_coords.height += 1;
        }

        Self {
            num_cols: max_rows,
            num_rows: max_cols,
            cells,
            box_coords,
            table_coords,
            ruler_coords,
            caption_coords,
        }
    }
}


#[allow(dead_code)]
pub struct TableCell {
    pub content: String,                // Content, will adhere to \n for line breaks
    pub wrapping: bool,                 // Any text should either wrap to next line or increases cell width
    pub h_alignment: TableHalign,       // Horizontal alignment: (L)eft (R)ight (C)enter
    pub v_alignment: TableValign,       // Vertical alignment: (T)op (B)ottom (C)enter
    pub colspan: usize,                 // This cell spans X cols
    pub rowspan: usize,                 // This cell spans X rows
}

#[allow(dead_code)]
pub struct TableRow {
    pub cells: Vec<TableCell>,      // Number of cells in the row. Does not have to be equal for all rows
}

#[allow(dead_code)]
pub struct Table {
    summary: String,                // Screen reader summary
    caption: String,                // Caption of the table, if set it will be displayed
    caption_pos: TableCaptionPos,   // Position of the caption
    direction: TableDir,            // LTR or RTL direction
    head_rows: Vec<TableRow>,       // <THEAD> rows
    body_rows: Vec<TableRow>,       // <TBODY> (default) rows
    footer_rows: Vec<TableRow>,     // <TFOOT> rows
    bordered: bool,                 // Cells are bordered or not
    cell_spacing: usize,            // Spacing between cells
    cell_padding: usize,            // Padding inside cells
    ruler: bool,                    // Render a ruler for the table
}

#[allow(dead_code)]
impl Table {
    pub fn new(ruler: bool) -> Self {
        Self {
            summary: String::new(),
            caption: String::new(),
            caption_pos: TableCaptionPos::TOP,
            direction: TableDir::LTR,
            head_rows: Vec::new(),
            body_rows: Vec::new(),
            footer_rows: Vec::new(),
            bordered: false,
            cell_spacing: 0,
            cell_padding: 0,
            ruler,
        }
    }

    pub fn with_summary(mut self, summary: &str) -> Self {
        self.summary = summary.into();
        self
    }

    pub fn with_caption(mut self, caption: &str, position: TableCaptionPos) -> Self {
        self.caption = caption.into();
        self.caption_pos = position;
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

        let mut output = StringRenderer::new(tv.box_coords.width, tv.box_coords.height);
        if self.ruler {
            self.render_ruler(&mut output, &tv);
        }
        if !self.caption.is_empty() {
            self.render_caption(&mut output, &tv);
        }

        if self.bordered {
            self.render_bordered(&mut output, &tv);
        } else {
            self.render_unbordered(&output, &tv);
        }

        output.render()
    }

    fn render_caption(&self, renderer: &mut impl TableRender, vars: &TableVars) {
        if vars.caption_coords.is_none() {
            return;
        }

        let c = vars.caption_coords.unwrap();

        let mut caption = self.caption.clone();
        if caption.len() > c.width {
            // Caption is too long, truncate it
            caption = caption[0..c.width].to_string();
        }

        let total_padding = c.width - caption.len();
        let left_pad = total_padding / 2;

        renderer.put_str(left_pad, c.y, &caption);
    }

    fn render_bordered(&self, renderer: &mut impl TableRender, vars: &TableVars) {
        // Render each cell border
        for cell in &vars.cells {
            self.render_border_box(renderer, vars.table_coords, cell.coord, BorderStyle::single());
        }

        // // Main table border
        // self.render_border_box(renderer, vars.box_coords, vars.table_coords, BorderStyle::double());
    }

    fn render_unbordered(&self, _renderer: &impl TableRender, _vars: &TableVars) -> String {
        let output = String::new();
        // output.push_str(self.render_row(&self.head_rows).as_str());
        // output.push_str(self.render_row(&self.body_rows).as_str());
        // output.push_str(self.render_row(&self.footer_rows).as_str());
        output
    }

    fn render_border_box(&self, renderer: &mut impl TableRender, parent_coord: Coord, box_coord: Coord, border: BorderStyle) {
        let c = parent_coord + box_coord;
        dbg!(&c);

        renderer.put_ch(c.x, c.y, border.top_left);
        renderer.put_ch(c.x + c.width - 1, c.y, border.top_right);
        renderer.put_ch(c.x, c.y + c.height - 1, border.bottom_left);
        renderer.put_ch(c.x + c.width - 1, c.y + c.height - 1, border.bottom_right);

        for i in 1..(c.width - 1) {
            renderer.put_ch(c.x + i, c.y, border.horizontal);
            renderer.put_ch(c.x + i, c.y + c.height - 1, border.horizontal);
        }

        for i in 1..(c.height - 1) {
            renderer.put_ch(c.x, c.y + i, border.vertical);
            renderer.put_ch(c.x + c.width - 1, c.y + i, border.vertical);
        }
    }

    fn render_ruler(&self, renderer: &mut impl TableRender, vars: &TableVars) {
        // Render a ruler for the table:
        //
        //      0         1         2         3         4         5         6
        //      0123456789012345678901234567890123456789012345678901234567890123456789

        if vars.ruler_coords.is_none() {
            return;
        }

        let c = vars.box_coords + vars.ruler_coords.unwrap();

        let mut ruler_top = String::new();
        let mut ruler_bottom = String::new();

        let width = vars.box_coords.width;

        for i in 0..(width / 10) {
            let number = format!("{:<width$}", i, width = 10);
            ruler_top.push_str(&number);
        }

        for i in 0..width {
            ruler_bottom.push_str(&(i % 10) .to_string());
        }

        renderer.put_str(0, c.y, &ruler_top);
        renderer.put_str(0, c.y+1, &ruler_bottom);
    }
}


/// Returns a vector with the width of each cell in the row based on the width and the number of columns.
fn calc_cell_width(width: usize, num_cols: usize) -> Vec<usize> {
    let mut cell_widths = Vec::new();
    let cell_width = (width-1) / num_cols;
    let remainder = (width-1) % num_cols;

    for _ in 0..num_cols {
        cell_widths.push(cell_width);
    }

    // Divide the remainder of the first cells
    for i in 0..remainder {
        cell_widths[i] += 1;
    }

    cell_widths
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
    fn test_table() {
        let mut t = Table::new(true)
            .with_summary("This is the summary of the table")
            .with_caption("This is the caption of the table", TableCaptionPos::TOP)
            .with_bordered(true)
            .with_cell_spacing(0)
            .with_cell_padding(0)
        ;

        t.add_head_row(TableRow {
            cells: vec![
                TableCell {
                    content: "Head 1".to_string(),
                    wrapping: false,
                    h_alignment: TableHalign::LEFT,
                    v_alignment: TableValign::TOP,
                    colspan: 1,
                    rowspan: 1,
                },
                TableCell {
                    content: "Head 2".to_string(),
                    wrapping: false,
                    h_alignment: TableHalign::RIGHT,
                    v_alignment: TableValign::BOTTOM,
                    colspan: 1,
                    rowspan: 1,
                },
            ]
        });

        t.add_body_row(TableRow {
            cells: vec![
                TableCell {
                    content: "Body 1".to_string(),
                    wrapping: false,
                    h_alignment: TableHalign::LEFT,
                    v_alignment: TableValign::TOP,
                    colspan: 1,
                    rowspan: 1,
                },
                TableCell {
                    content: "Body 2".to_string(),
                    wrapping: false,
                    h_alignment: TableHalign::LEFT,
                    v_alignment: TableValign::TOP,
                    colspan: 1,
                    rowspan: 1,
                },
            ]
        });

        t.add_body_row(TableRow {
            cells: vec![
                TableCell {
                    content: "Body 3".to_string(),
                    wrapping: false,
                    h_alignment: TableHalign::RIGHT,
                    v_alignment: TableValign::BOTTOM,
                    colspan: 1,
                    rowspan: 1,
                },
                TableCell {
                    content: "Body 4\nwith some extra\ntext".to_string(),
                    wrapping: false,
                    h_alignment: TableHalign::LEFT,
                    v_alignment: TableValign::TOP,
                    colspan: 1,
                    rowspan: 1,
                },
            ]
        });

        t.add_footer_row(TableRow {
            cells: vec![
                TableCell {
                    content: "Footer 1".to_string(),
                    wrapping: false,
                    h_alignment: TableHalign::LEFT,
                    v_alignment: TableValign::TOP,
                    colspan: 1,
                    rowspan: 1,
                },
                TableCell {
                    content: "Footer 2".to_string(),
                    wrapping: false,
                    h_alignment: TableHalign::LEFT,
                    v_alignment: TableValign::TOP,
                    colspan: 1,
                    rowspan: 1,
                },
            ]
        });

        println!("TABLE START");
        println!("{}", t.render(100));
        println!("TABLE END");
    }

    #[test]
    fn test_box_renderer() {
        let mut sr = StringRenderer::new(20, 20);


        let table = Table::new(false);

        table.render_border_box(&mut sr, Coord::new(0, 0, 20, 20), Coord::new(5, 5, 10, 10), BorderStyle::single());
        table.render_border_box(&mut sr, Coord::new(0, 0, 20, 20), Coord::new(4, 4, 12, 12), BorderStyle::double());

        println!("{}", sr.render());
    }

    #[test]
    fn test_string_renderer() {
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

        println!("{}", sr.render());
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

/*
0         1         2         3         4         5         6         7         8         9        9
0123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789
|                                                |                                                 |

+-------------------+-------------------+-------------------+------------------+
|                   |                   |                   |                  |
|1234567890123456789|1234567890123456789|1234567890123456789|123456789012345678|
+------------------------------------------------------------------------------+
|                          |                         |                         |
|12345678901234567890123456|1234567890123456789012345|1234567890123456789012345|
+------------------------------------------------------------------------------+
|                                       |                                      |
|123456789012345678901234567890123456789|12345678901234567890123456789012345678|
+------------------------------------------------------------------------------+
|               |               |               |               |              |
|123456789012345|123456789012345|123456789012345|123456789012345|12345678901234|
+------------------------------------------------------------------------------+
|          |          |          |          |           |           |          |
|1234567890|1234567890|1234567890|1234567890|12345678901|12345678901|1234567890|
+------------------------------------------------------------------------------+

|2345678901234567890                    |2345678901234567890
                    |2345678901234567890                    |234567890123456789|
 */


    #[test]
    fn test_calc_cell_width() {
        assert_eq!(vec!(40, 39), calc_cell_width(80, 2));
        assert_eq!(vec!(27, 26, 26), calc_cell_width(80, 3));
        assert_eq!(vec!(20, 20, 20, 19), calc_cell_width(80, 4));
        assert_eq!(vec!(16, 16, 16, 16, 15), calc_cell_width(80, 5));
        assert_eq!(vec!(14, 13, 13, 13, 13, 13), calc_cell_width(80, 6));
        assert_eq!(vec!(12, 12, 11, 11, 11, 11, 11), calc_cell_width(80, 7));

        assert_eq!(vec!(21, 20, 20, 20), calc_cell_width(82, 4));
        assert_eq!(vec!(20, 20, 20, 20), calc_cell_width(81, 4));
        assert_eq!(vec!(20, 20, 20, 19), calc_cell_width(80, 4));
        assert_eq!(vec!(20, 20, 19, 19), calc_cell_width(79, 4));
        assert_eq!(vec!(20, 19, 19, 19), calc_cell_width(78, 4));
        assert_eq!(vec!(19, 19, 19, 19), calc_cell_width(77, 4));
        assert_eq!(vec!(19, 19, 19, 18), calc_cell_width(76, 4));
    }

    #[test]
    fn test_coords() {
        let c1 = Coord::new(0, 0, 10, 5);
        let c2 = Coord::new(5, 5, 10, 5);
        assert_eq!(Coord::new(5, 5, 10, 5), c1 + c2);

        let c1 = Coord::new(0, 0, 10, 5);
        let c2 = Coord::new(5, 5, 20, 5);
        assert_eq!(Coord::new(5, 5, 20, 5), c1 + c2);

        let c1 = Coord::new(0, 0, 20, 4);
        let c2 = Coord::new(5, 5, 10, 50);
        assert_eq!(Coord::new(5, 5, 20, 50), c1 + c2);

        let c1 = Coord::new(0, 0, 10, 5);
        let c2 = Coord::new(0, 5, 10, 5);
        assert_eq!(Coord::new(0, 10, 10, 5), c1 + c2 + c2);
        assert_eq!(Coord::new(0, 15, 10, 5), c1 + c2 + c2 + c2);
    }
}




/*


0 +---------------
1 |
2 +---------------
3 |
4 +---------------
5 |
6 +---------------
 */