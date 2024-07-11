use std::cmp;
use std::collections::HashMap;
use std::ops::Add;

/// A table renderer is a trait that can render a table to something. It can either be a string
/// type system, or something graphical.
trait TableRender {
    /// Creates a new renderer that can generate the table. Note that we need to know the width and
    /// height of the table in advance.
    fn new(height: usize, width: usize) -> impl TableRender;
    /// Puts a string at the given position
    fn put_str(&mut self, x: usize, y: usize, content: &str);
    /// Puts a single character at the given position
    fn put_ch(&mut self, x: usize, y: usize, c: char);
    /// Retrieves a character from the buffer
    fn get_ch(&self, x: usize, y: usize) -> char;
    /// Do the actual rendering (if needed) and return the result
    fn render(&self) -> String;
}

/// The StringRenderer allows you to render a table into a string which can
/// be printed onto the screen
struct StringRenderer {
    width: usize,
    height: usize,
    buf: Vec<char>,
}

impl TableRender for StringRenderer {
    /// Creates a new string renderer with the given width and height
    fn new(width: usize, height: usize) -> StringRenderer {
        Self{
            width,
            height,
            buf: vec!(' '; width * height),
        }
    }

    /// Puts a string at the given position
    fn put_str(&mut self, x: usize, y: usize, content: &str) {
        // Note that put_str will overflow into a new line if the end of the line is reached.
        for i in 0..content.len() {
            self.put_ch(x + i, y, content.chars().nth(i).unwrap());
        }
    }

    /// Puts a single character at the given position
    fn put_ch(&mut self, x: usize, y: usize, c: char) {
        // Make sure we are still in range of the buffer
        let idx = y * self.width + x;
        if idx >= self.buf.len() {
            return;
        }

        self.buf[idx] = c;
    }

    /// Returns the character at the given position
    fn get_ch(&self, x: usize, y: usize) -> char {
        let idx = y * self.width + x;
        self.buf[idx]
    }

    /// Renders the buffer into a string
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

/// Border style allows you to set the characters used for the borders of the table. Note that
/// mixing border styles do not work well.
#[allow(dead_code)]
struct BorderStyle {
    pub horizontal: char,
    pub vertical: char,
    pub top_left: char,
    pub top_right: char,
    pub bottom_left: char,
    pub bottom_right: char,
    pub cross: char,
    pub t_down: char,
    pub t_up: char,
    pub t_left: char,
    pub t_right: char,
    // Junctions are an easy way to replace a character for the correct one once a cell is rendered
    // on top of another cell (for instance, when merging the border). Each junction is a part that
    // will result in a different character.
    pub junctions: HashMap<(char, char), char>,
}

impl BorderStyle {
    fn single() -> Self {
        let mut style = Self {
            top_left: '┌',
            top_right: '┐',
            bottom_left: '└',
            bottom_right: '┘',
            horizontal: '─',
            vertical: '│',
            cross: '┼',
            t_down: '┬',
            t_up: '┴',
            t_left: '┤',
            t_right: '├',
            junctions: HashMap::new(),
        };

        style.junctions.insert((' ', style.horizontal), style.horizontal);
        style.junctions.insert((' ', style.vertical), style.vertical);
        style.junctions.insert((style.horizontal, style.vertical), style.cross);
        style.junctions.insert((style.horizontal, style.top_left), style.t_down);
        style.junctions.insert((style.horizontal, style.top_right), style.t_down);
        style.junctions.insert((style.horizontal, style.bottom_left), style.t_up);
        style.junctions.insert((style.horizontal, style.bottom_right), style.t_up);
        style.junctions.insert((style.vertical, style.top_left), '├');
        style.junctions.insert((style.vertical, style.top_right), '┤');
        style.junctions.insert((style.vertical, style.bottom_left), '├');
        style.junctions.insert((style.vertical, style.bottom_right), '┤');
        style.junctions.insert((style.top_left, style.top_right), style.horizontal);
        style.junctions.insert((style.bottom_left, style.bottom_right), style.horizontal);
        style.junctions.insert((style.top_left, style.bottom_left), style.vertical);
        style.junctions.insert((style.top_right, style.bottom_right), style.vertical);

        style
    }

    fn double() -> Self {
        let mut style = Self {
            top_left: '╔',
            top_right: '╗',
            bottom_left: '╚',
            bottom_right: '╝',
            horizontal: '═',
            vertical: '║',
            cross: '╬',
            t_down: '╦',
            t_up: '╩',
            t_left: '╣',
            t_right: '╠',
            junctions: HashMap::new(),
        };

        // style.junctions.insert((' ', style.horizontal), style.horizontal);
        // style.junctions.insert((' ', style.vertical), style.vertical);
        style.junctions.insert((style.horizontal, style.vertical), style.cross);
        style.junctions.insert((style.horizontal, style.top_left), style.t_down);
        style.junctions.insert((style.horizontal, style.top_right), style.t_down);
        style.junctions.insert((style.horizontal, style.bottom_left), style.t_up);
        style.junctions.insert((style.horizontal, style.bottom_right), style.t_up);
        style.junctions.insert((style.vertical, style.top_left), style.t_right);
        style.junctions.insert((style.vertical, style.top_right), style.t_left);
        style.junctions.insert((style.vertical, style.bottom_left), style.t_right);
        style.junctions.insert((style.vertical, style.bottom_right), style.t_left);
        style.junctions.insert((style.top_left, style.top_right), style.t_down);
        style.junctions.insert((style.bottom_left, style.bottom_right), style.t_up);
        style.junctions.insert((style.top_left, style.bottom_left), style.t_right);
        style.junctions.insert((style.top_right, style.bottom_right), style.t_left);

        style.junctions.insert((style.top_left, style.bottom_right), style.cross);
        style.junctions.insert((style.bottom_left, style.top_right), style.cross);

        style.junctions.insert((style.t_right, style.horizontal), style.cross);
        style.junctions.insert((style.t_right, style.top_right), style.cross);
        style.junctions.insert((style.t_right, style.bottom_left), style.cross);
        style.junctions.insert((style.t_left, style.horizontal), style.cross);
        style.junctions.insert((style.t_left, style.top_left), style.cross);
        style.junctions.insert((style.t_left, style.bottom_right), style.cross);
        style.junctions.insert((style.t_up, style.vertical), style.cross);
        style.junctions.insert((style.t_up, style.top_left), style.cross);
        style.junctions.insert((style.t_up, style.top_right), style.cross);
        style.junctions.insert((style.t_down, style.vertical), style.cross);
        style.junctions.insert((style.t_down, style.bottom_left), style.cross);
        style.junctions.insert((style.t_down, style.bottom_right), style.cross);

        style
    }
}

/// A rect is a rectangle box starting at X,Y with H,W dimensions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// X position
    pub x: usize,
    /// Y position
    pub y: usize,
    /// Width
    pub width: usize,
    /// Height
    pub height: usize,
}

impl Rect {
    pub fn new(x: usize, y: usize, width: usize, height: usize) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

impl Add for Rect {
    type Output = Rect;

    /// Rects can be added onto each other, sort of.
    fn add(self, rhs: Self) -> Self::Output {
        Rect {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            width: rhs.width,
            height: rhs.height,
        }
    }
}

/// Table direction. To which side are cells added
#[derive(Debug)]
pub enum TableDir {
    /// Table is left-to-right
    LTR,
    /// Table is right-to-left
    RTL,
}

/// Horizontal alignment of text in a cell
#[derive(Debug)]
pub enum TableHalign {
    /// Align text to the left
    LEFT,
    /// Align text to the right
    RIGHT,
    /// Align text in the center
    CENTER,
}

/// Vertical alignment of text in a cell
#[derive(Debug)]
pub enum TableValign {
    /// Align text to the right
    TOP,
    /// Align text to the bottom
    BOTTOM,
    /// Align text to the center
    CENTER,
}

#[derive(Debug, PartialEq)]
pub enum TableCaptionPos {
    /// Caption should be printed above the table
    TOP,
    /// Caption should be printed below the table
    BOTTOM,
}

/// TableVars hold all precalculated variables of a table in order to render it.
#[allow(dead_code)]
#[derive(Debug)]
struct TableVars {
    /// Maximum number of columns in the table
    num_cols: usize,
    /// Number of rows in the table
    num_rows: usize,
    /// Number of header rows
    num_header_rows: usize,
    /// Number of body rows
    num_body_rows: usize,
    /// Number of footer rows
    num_footer_rows: usize,
    /// Data for each cell found in the table
    cells: Vec<TableCellVars>,
    /// Total rectangle of the table including all parts like rulers and captions
    box_rect: Rect,
    /// Box rect of the actual table
    table_rect: Rect,
    /// Box rectangle of the ruler, if any
    ruler_rect: Option<Rect>,
    /// Box rectangle of the caption, if any
    caption_rect: Option<Rect>,
}

/// TableCellVars holds all precalculated variables of a table cell in order to render it.
#[allow(dead_code)]
#[derive(Debug)]
struct TableCellVars {
    /// Dimension and position of the cell (already includes row/colspans)
    rect: Rect,
    // Cell is bordered or not
    bordered: bool,
    // Actual content to be rendered inside the cell
    content: String,
    // Is this cell part of a tbody, thead, or tfoot
    section: TableSection,
}

impl TableVars {
    /// Calculates the maximum number of rows and cols based on the rows in the table. This is
    /// somewhat complex in order to deal with rowspans and colspans.
    fn calculate_dimensions(rows: &Vec<TableRow>) -> (usize, usize) {
        let mut max_rows = 0;
        let mut max_cols = 0;

        let mut col_tracker: Vec<usize> = vec![];

        for (row_index, row) in rows.iter().enumerate() {
            let mut current_col = 0;

            for cell in row.cells.iter() {
                while current_col < col_tracker.len() && col_tracker[current_col] > row_index {
                    current_col += 1;
                }

                max_cols = max_cols.max(current_col + cell.colspan);

                for _ in 0..cell.colspan {
                    if col_tracker.len() <= current_col {
                        col_tracker.push(row_index + cell.rowspan);
                    } else {
                        col_tracker[current_col] = row_index + cell.rowspan;
                    }
                    current_col += 1;
                }
            }

            max_rows = max_rows.max(row_index + 1);
        }

        for &row_end in &col_tracker {
            max_rows= max_rows.max(row_end);
        }

        (max_rows, max_cols)
    }

    /// This will compute all the variables needed to render the table in the given box_width.
    fn compute(table: &Table, box_width: usize) -> Self {

        ////////////////
        // Calculate row/col dimensions for each tbody, tfoot, thead. Each of them are calculated
        // separately, but the maximum cols are used for the final table.
        let mut max_cols = 0;

        let (_, col_cnt) = Self::calculate_dimensions(&table.thead_rows);
        max_cols = max_cols.max(col_cnt);

        let (_, col_cnt) = Self::calculate_dimensions(&table.tbody_rows);
        max_cols = max_cols.max(col_cnt);

        let (_, col_cnt) = Self::calculate_dimensions(&table.tfoot_rows);
        max_cols = max_cols.max(col_cnt);

        // Max rows cannot extend the number of rows in the table, not even with rowspan, so we can simply add them together.
        let max_rows = table.thead_rows.len() + table.tbody_rows.len() + table.tfoot_rows.len();


        ////////////////
        // Step: Setup rects for the different elements of the table
        let mut box_rect = Rect::new(0, 0, cmp::max(3, box_width), 0);
        let mut table_rect = Rect::new(0, 0, cmp::max(3, box_width), 0);
        let mut caption_rect = None;
        let mut ruler_rect = None;


        ////////////////
        // Step: Create the actual table

        // We have max_cols and max_rows. It needs to fit in width chars, so calculate what each row width
        // should be (including borders)
        let cell_widths = calc_cell_width(box_width, max_cols);

        // Calculate height / offset for each cell, and update table rects
        let cell_height;
        let cell_offset;
        if table.bordered {
            // Make sure we add some height for the borders. Offset is basically cell_height - 1, as this keeps
            // the offset where the next cell should be rendered (this should be on top of the bottom border of the previous row)
            cell_height = 3;        // Each cell is 3 lines high
            cell_offset = 2;        // But start at offset 2, so they will overlap with the top of the previous cell

            // Increase height of table and box with the number of rows, including the borders and the fact that borders overlap
            table_rect.height += 2 * max_rows + 1;
            box_rect.height += 2 * max_rows + 1;

        } else {
            cell_height = 1;
            cell_offset = 1;

            // Increase height of table and box with the number of rows
            table_rect.height += max_rows;
            box_rect.height += max_rows;
        }

        // This array contains all the cells that are occupied by a cell, or by a rowspan or colspan
        let mut occupied_cells = vec![vec![false; max_cols]; max_rows];

        // This array keeps all the rendered tablecellvars
        let mut cells = Vec::new();

        // Src is the pointer to current cell in the table (ROW/COL)
        let mut src_idx = (0, 0);
        // Dst is the pointer to the output cell position, which is not always a 1-1 mapping in case of row/colspans (ROW/COL)
        let mut dst_idx = (0, 0);

        // Just iterate all rows and cols. It is very possible that not all of them result in a valid cell in case of row/colspans.
        for _ in 0..max_rows {
            for _ in 0..max_cols {
                // Fetch the next cell from our table
                let table_cell = table.get_cell(src_idx.0, src_idx.1);
                if table_cell.is_none() {
                    // This cell does not exist. That's fine and we can just break out the loop onto the next row
                    break;
                }
                let table_cell = table_cell.unwrap();

                // We need to know if this cell is in thead, tbody or tfoot.
                let cell_section = table.get_section(src_idx.0);

                // We use our occupied_cells matrix to find the next destination spot for this cell.
                while occupied_cells[dst_idx.0][dst_idx.1] {
                    dst_idx.1 += 1;
                    if dst_idx.1 >= max_cols {
                        // Max cols should already take into account any colspans, so this cannot happen.
                        panic!("This should not happen.")
                    }
                }

                // Find the actual X and Y position for the given destination cell
                let mut cell_x = 0;
                for i in 0..dst_idx.1 {
                    // Just add the width of the cell + 1 for the border
                    cell_x += cell_widths[i] + 1;
                }
                let cell_y = cell_offset * dst_idx.0;

                //////////////
                // Expand width of the cell based on the colspan and rowspan

                // Make sure we are never go beyond the max rows of the given section of the table (thead, tbody, tfoot)
                // This means that when the last row in a tbody has a rowspan of 10, it still will only render 1 row.
                let mut capped_rowspan = table_cell.rowspan;
                let (section_start_row, section_row_count) = table.get_section_boundaries(cell_section);
                if src_idx.0 + capped_rowspan - 1 >= section_start_row + section_row_count {
                    capped_rowspan = section_start_row + section_row_count - src_idx.0;
                }

                // Mark all destination cells that this cell spans as occupied so we can skip this cell
                // as a destination
                for i in 0..capped_rowspan {
                    for j in 0..table_cell.colspan {
                        occupied_cells[dst_idx.0 + i][dst_idx.1 + j] = true;
                    }
                }


                let cell_height = cell_height * capped_rowspan - (capped_rowspan - 1);
                let mut cell_width= 0;
                for i in 0..table_cell.colspan {
                    // Width of the cell is the number of spans it has, plus two chars extra for the border
                    cell_width += cell_widths[dst_idx.1 + i] + 2;
                }
                // And we need to remove the border
                cell_width -= table_cell.colspan-1;

                // Push all our calculated info into a list of TableCellsVars
                cells.push(TableCellVars {
                    rect: Rect {
                        x: cell_x,
                        y: cell_y,
                        width: cell_width,
                        height: cell_height,
                    },
                    bordered: table.bordered,
                    content: table_cell.content.clone(),
                    section: cell_section,
                });

                // Next src/dst pointer
                src_idx.1 += 1;
                dst_idx.1 += 1;
            }

            // Next destination row, start at column 0
            dst_idx.0 += 1;
            dst_idx.1 = 0;

            // Next source row and start at column 0
            src_idx.0 += 1;
            src_idx.1 = 0;
        }

        ////////////////
        // Step: Add additional ruler and caption

        // Todo: adding ruler and caption is pretty messy. We manually have to increase
        // other rects in order make room for the new elements. We should have some kind of
        // list of rects that can be added to the table, and then we can just iterate over them.
        // This might mean we need some kind of rect-manager, which might be a bit too much for now.

        if table.ruler {
            ruler_rect = Some(Rect::new(0, 0, box_rect.width, 2));
            // Move up the table and increase height to make room for the ruler
            table_rect.y += 2;
            box_rect.height += 2;
        }

        if !table.caption.is_empty() && table.caption_pos == TableCaptionPos::TOP {
            if ruler_rect.is_none() {
                // No ruler, so caption is at the top of the table
                caption_rect = Some(Rect::new(0, 0, box_rect.width, 1));
            } else {
                caption_rect = Some(Rect::new(0, 2, box_rect.width, 1));
            }
            table_rect.y += 1;
            box_rect.height += 1;
        }

        if table.caption_pos == TableCaptionPos::BOTTOM {
            // Caption is at the bottom, so only increase height
            caption_rect = Some(Rect::new(0, box_rect.height, box_rect.width, 1));
            box_rect.height += 1;
        }

        Self {
            num_cols: max_cols,
            num_rows: max_rows,
            num_header_rows: table.thead_rows.len(),
            num_body_rows: table.tbody_rows.len(),
            num_footer_rows: table.tfoot_rows.len(),
            cells,
            box_rect,
            table_rect,
            ruler_rect,
            caption_rect,
        }
    }
}

/// A cell can be in any of these three sections
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TableSection {
    Header,
    Body,
    Footer,
}

/// A tablecell is a single cell in a table. It can span multiple rows and columns, have alignment and content.
#[allow(dead_code)]
#[derive(Debug)]
pub struct TableCell {
    /// Content, will adhere to \n for line breaks
    pub content: String,
    /// Any text should either wrap to next line or increases cell width
    pub wrapping: bool,
    /// Horizontal alignment: (L)eft (R)ight (C)enter
    pub h_alignment: TableHalign,
    /// Vertical alignment: (T)op (B)ottom (C)enter
    pub v_alignment: TableValign,
    /// This cell spans X cols
    pub colspan: usize,
    /// This cell spans X rows
    pub rowspan: usize,
}

/// A table row is a simple list of cells. The number of cells in a row does not have to be equal for all rows.
#[allow(dead_code)]
pub struct TableRow {
    /// Number of cells in the row. Does not have to be equal for all rows
    pub cells: Vec<TableCell>,
}

/// A table is a collection of rows, with a summary, caption, and other settings.
#[allow(dead_code)]
pub struct Table {
    /// Screen reader summary
    summary: String,
    /// Caption of the table, if set it will be displayed
    caption: String,
    /// Position of the caption
    caption_pos: TableCaptionPos,
    /// LTR or RTL direction
    direction: TableDir,
    /// Defined rows for thead
    thead_rows: Vec<TableRow>,
    /// Defined rows for tbody (or default)
    tbody_rows: Vec<TableRow>,
    /// Defined rows for tfoot
    tfoot_rows: Vec<TableRow>,
    /// Cells are bordered or not
    bordered: bool,
    /// Spacing between cells
    cell_spacing: usize,
    /// Padding inside cells
    cell_padding: usize,
    /// Render a ruler for the table
    ruler: bool,
}

impl Table {
    /// Get the cell based on index number, and spans thead, tbody and tfoot. Return None if the cell
    /// does not exist (out of range)
    pub(crate) fn get_cell(&self, row_idx: usize, col_idx: usize) -> Option<&TableCell> {
        let row;

        if row_idx < self.thead_rows.len() {
            row = self.thead_rows.get(row_idx).unwrap();
        } else if row_idx < self.thead_rows.len() + self.tbody_rows.len() {
            row = self.tbody_rows.get(row_idx - self.thead_rows.len()).unwrap();
        } else {
            row = self.tfoot_rows.get(row_idx - self.thead_rows.len() - self.tbody_rows.len()).unwrap();
        }

        row.cells.get(col_idx)
    }

    /// Returns the table section of a row. This is because we threath all rows as consecutive but sometimes
    /// we need to know if a cell (row) is part of the thead, tbody or tfoot.
    pub(crate) fn get_section(&self, row_idx: usize) -> TableSection {
        return if row_idx < self.thead_rows.len() {
            TableSection::Header
        } else if row_idx < self.thead_rows.len() + self.tbody_rows.len() {
            TableSection::Body
        } else {
            TableSection::Footer
        }
    }

    /// Returns the starting row and the number of rows in a given section. Needed so we can cap rowspans since they
    /// cannot exceed the number of rows in the section.
    pub(crate) fn get_section_boundaries(&self, section: TableSection) -> (usize, usize) {
        match section {
            TableSection::Header => (0, self.thead_rows.len()),
            TableSection::Body => (0 + self.thead_rows.len(), self.tbody_rows.len()),
            TableSection::Footer => (0 + self.thead_rows.len() + self.tbody_rows.len(), self.tfoot_rows.len()),
        }
    }
}

#[allow(dead_code)]
impl Table {
    pub fn new(ruler: bool) -> Self {
        Self {
            summary: String::new(),
            caption: String::new(),
            caption_pos: TableCaptionPos::TOP,
            direction: TableDir::LTR,
            thead_rows: Vec::new(),
            tbody_rows: Vec::new(),
            tfoot_rows: Vec::new(),
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

    pub fn add_header_row(&mut self, row: TableRow) {
        self.thead_rows.push(row);
    }

    pub fn add_body_row(&mut self, row: TableRow) {
        self.tbody_rows.push(row);
    }

    pub fn add_footer_row(&mut self, row: TableRow) {
        self.tfoot_rows.push(row);
    }

    /// Renders the table into a string with the given width
    pub fn render(&self, width: usize) -> String {
        let width = cmp::max(3, width);
        let tv = TableVars::compute(self, width);

        let mut output = StringRenderer::new(tv.box_rect.width, tv.box_rect.height);
        if self.ruler {
            self.render_ruler(&mut output, &tv);
        }
        if !self.caption.is_empty() {
            self.render_caption(&mut output, &tv);
        }

        if self.bordered {
            self.render_bordered(&mut output, &tv);
        } else {
            self.render_unbordered(&mut output, &tv);
        }

        output.render()
    }

    /// Render a caption (if any)
    fn render_caption(&self, renderer: &mut impl TableRender, vars: &TableVars) {
        if vars.caption_rect.is_none() {
            return;
        }

        let c = vars.caption_rect.unwrap();

        let mut caption = self.caption.clone();
        if caption.len() > c.width {
            // Caption is too long, truncate it
            caption = caption[0..c.width].to_string();
        }

        let total_padding = c.width - caption.len();
        let left_pad = total_padding / 2;

        renderer.put_str(left_pad, c.y, &caption);
    }

    /// Render the actual table as bordered
    fn render_bordered(&self, renderer: &mut impl TableRender, vars: &TableVars) {
        // Render each cell border
        for cell in &vars.cells {
            // Headers and footers might get different styles
            let mut style = BorderStyle::double();
            if cell.section == TableSection::Header || cell.section == TableSection::Footer {
                style = BorderStyle::double();
            }

            // Make sure the used rect is based on the start of the table
            let c = vars.box_rect + vars.table_rect + cell.rect;
            self.render_border_box(renderer, c, &style);

            renderer.put_str(c.x + 1, c.y + 1, &cell.content);
        }

        // Render main table border on top of the cells
        self.render_border_box(renderer, vars.box_rect + vars.table_rect, &BorderStyle::double());
    }

    /// Render the actual table as unbordered
    fn render_unbordered(&self, renderer: &mut impl TableRender, vars: &TableVars) {
        // Render each cell border
        for cell in &vars.cells {
            let c = vars.box_rect + vars.table_rect + cell.rect;
            renderer.put_str(c.x + 1, c.y + 1, &cell.content);
        }
    }

    // Render a single box based on the rect
    fn render_border_box(&self, renderer: &mut impl TableRender, rect: Rect, style: &BorderStyle) {
        fn render_box_char(renderer: &mut impl TableRender, x: usize, y: usize, c: char, style: &BorderStyle) {
            let ch = renderer.get_ch(x, y);
            if ch == ' ' {
                // Nothing here yet, so just add the character
                renderer.put_ch(x, y, c);
                return;
            }

            // Create a pair of the current character and the new character
            let pair = (renderer.get_ch(x, y), c);

            if pair.0 == pair.1 {
                // It's the same char, so do nothing.
                return;
            }


            if let Some(&new_ch) = style.junctions.get(&pair) {
                // Found this combination in the junctions, so replace the character
                renderer.put_ch(x, y, new_ch);
            } else if let Some(&new_ch) = style.junctions.get(&(pair.1, pair.0)) {
                // Found this combination (as reversed) in the junctions, so replace the character
                renderer.put_ch(x, y, new_ch);
            }
        }

        let x1 = rect.x;
        let x2 = rect.x + rect.width - 1;
        let y1 = rect.y;
        let y2 = rect.y + rect.height - 1;

        // Horizontal bars (skip corners)
        for x in x1+1..x2 {
            render_box_char(renderer, x, y1, style.horizontal, &style);
            render_box_char(renderer, x, y2, style.horizontal, &style);
        }
        // Vertical bars (skip corners)
        for y in y1+1..y2 {
            render_box_char(renderer, x1, y, style.vertical, &style);
            render_box_char(renderer, x2, y, style.vertical, &style);
        }

        // Render the corners
        render_box_char(renderer, x1, y1, style.top_left, &style);
        render_box_char(renderer, x2, y1, style.top_right, &style);
        render_box_char(renderer, x1, y2, style.bottom_left, &style);
        render_box_char(renderer, x2, y2, style.bottom_right, &style);
    }

    /// Render a ruler
    fn render_ruler(&self, renderer: &mut impl TableRender, vars: &TableVars) {
        // Render a ruler for the table:
        //
        //      0         1         2         3         4         5         6
        //      0123456789012345678901234567890123456789012345678901234567890123456789

        if vars.ruler_rect.is_none() {
            return;
        }

        let c = vars.box_rect + vars.ruler_rect.unwrap();
        let width = vars.box_rect.width;

        let mut ruler_top = String::new();
        let mut ruler_bottom = String::new();

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

/// Returns a vector with the width of each cell in the row based on the width and the
/// number of columns.
fn calc_cell_width(width: usize, num_cols: usize) -> Vec<usize> {
    let mut cell_widths = Vec::new();

    let cell_width = (width-num_cols-1) / num_cols;
    let remainder = (width-num_cols-1) % num_cols;

    for _ in 0..num_cols {
        cell_widths.push(cell_width);
    }

    // Divide the remainder of the first cells
    for i in 0..remainder {
        cell_widths[i] += 1;
    }

    cell_widths
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
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

        t.add_header_row(TableRow {
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
                TableCell {
                    content: "Head 3".to_string(),
                    wrapping: false,
                    h_alignment: TableHalign::RIGHT,
                    v_alignment: TableValign::BOTTOM,
                    colspan: 1,
                    rowspan: 1,
                },
                TableCell {
                    content: "Head 4".to_string(),
                    wrapping: false,
                    h_alignment: TableHalign::RIGHT,
                    v_alignment: TableValign::BOTTOM,
                    colspan: 1,
                    rowspan: 1,
                },
            ],
        });

        t.add_body_row(TableRow {
            cells: vec![
                TableCell {
                    content: "Body 1 C2R2".to_string(),
                    wrapping: false,
                    h_alignment: TableHalign::LEFT,
                    v_alignment: TableValign::TOP,
                    colspan: 2,
                    rowspan: 2,
                },
                TableCell {
                    content: "Body 2".to_string(),
                    wrapping: false,
                    h_alignment: TableHalign::LEFT,
                    v_alignment: TableValign::TOP,
                    colspan: 1,
                    rowspan: 1,
                },
            ],
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
                    // content: "Body 4\nwith some extra\ntext".to_string(),
                    content: "Body 4 C2".to_string(),
                    wrapping: false,
                    h_alignment: TableHalign::LEFT,
                    v_alignment: TableValign::TOP,
                    colspan: 2,
                    rowspan: 1,
                },
                TableCell {
                    // content: "Body 5 with a very long text on the same line".to_string(),
                    content: "Body 5 R2".to_string(),
                    wrapping: false,
                    h_alignment: TableHalign::LEFT,
                    v_alignment: TableValign::TOP,
                    colspan: 1,
                    rowspan: 2,
                },
            ],
        });

        t.add_body_row(TableRow {
            cells: vec![
                TableCell {
                    content: "Body 6".to_string(),
                    wrapping: false,
                    h_alignment: TableHalign::LEFT,
                    v_alignment: TableValign::TOP,
                    colspan: 1,
                    rowspan: 1,
                },
                TableCell {
                    content: "Body 7".to_string(),
                    wrapping: false,
                    h_alignment: TableHalign::LEFT,
                    v_alignment: TableValign::TOP,
                    colspan: 1,
                    rowspan: 1,
                },
            ],
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
            ],
        });

        println!("{}", t.render(100));
    }

    #[test]
    fn test_box_renderer() {
        let mut sr = StringRenderer::new(20, 20);

        let table = Table::new(false);
        table.render_border_box(&mut sr, Rect::new(5, 5, 10, 10), &BorderStyle::single());
        table.render_border_box(&mut sr, Rect::new(4, 4, 12, 12), &BorderStyle::double());

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

|1234567890123456789                    |1234567890123456789
                    |1234567890123456789                    |123456789012345678|
 */

    #[test]
    fn test_calc_cell_width() {
        assert_eq!(vec!(39, 38), calc_cell_width(80, 2));
        assert_eq!(vec!(26, 25, 25), calc_cell_width(80, 3));
        assert_eq!(vec!(19, 19, 19, 18), calc_cell_width(80, 4));
        assert_eq!(vec!(15, 15, 15, 15, 14), calc_cell_width(80, 5));
        assert_eq!(vec!(13, 12, 12, 12, 12, 12), calc_cell_width(80, 6));
        assert_eq!(vec!(11, 11, 10, 10, 10, 10, 10), calc_cell_width(80, 7));

        assert_eq!(vec!(20, 19, 19, 19), calc_cell_width(82, 4));
        assert_eq!(vec!(19, 19, 19, 19), calc_cell_width(81, 4));
        assert_eq!(vec!(19, 19, 19, 18), calc_cell_width(80, 4));
        assert_eq!(vec!(19, 19, 18, 18), calc_cell_width(79, 4));
        assert_eq!(vec!(19, 18, 18, 18), calc_cell_width(78, 4));
        assert_eq!(vec!(18, 18, 18, 18), calc_cell_width(77, 4));
        assert_eq!(vec!(18, 18, 18, 17), calc_cell_width(76, 4));
    }

    #[test]
    fn test_rect() {
        let c1 = Rect::new(0, 0, 10, 5);
        let c2 = Rect::new(5, 5, 10, 5);
        assert_eq!(Rect::new(5, 5, 10, 5), c1 + c2);

        let c1 = Rect::new(0, 0, 10, 5);
        let c2 = Rect::new(5, 5, 20, 5);
        assert_eq!(Rect::new(5, 5, 20, 5), c1 + c2);

        let c1 = Rect::new(0, 0, 20, 4);
        let c2 = Rect::new(5, 5, 10, 50);
        assert_eq!(Rect::new(5, 5, 10, 50), c1 + c2);

        let c1 = Rect::new(0, 0, 10, 5);
        let c2 = Rect::new(0, 5, 10, 5);
        assert_eq!(Rect::new(0, 10, 10, 5), c1 + c2 + c2);
        assert_eq!(Rect::new(0, 15, 10, 5), c1 + c2 + c2 + c2);
    }

    #[test]
    fn test_calculate_dimensions() {
        let mut t = Table::new(true)
            .with_bordered(true);

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
            ],
        });

        let (max_rows, max_cols) = TableVars::calculate_dimensions(&t.tbody_rows);
        assert_eq!(max_rows, 1);
        assert_eq!(max_cols, 2);


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
                TableCell {
                    content: "Body 2".to_string(),
                    wrapping: false,
                    h_alignment: TableHalign::LEFT,
                    v_alignment: TableValign::TOP,
                    colspan: 1,
                    rowspan: 1,
                },
            ],
        });

        let (max_rows, max_cols) = TableVars::calculate_dimensions(&t.tbody_rows);
        assert_eq!(max_rows, 2);
        assert_eq!(max_cols, 3);


        t.add_body_row(TableRow {
            cells: vec![
                TableCell {
                    content: "Body 1".to_string(),
                    wrapping: false,
                    h_alignment: TableHalign::LEFT,
                    v_alignment: TableValign::TOP,
                    colspan: 3,
                    rowspan: 2,
                },
            ],
        });

        let (max_rows, max_cols) = TableVars::calculate_dimensions(&t.tbody_rows);
        assert_eq!(max_rows, 4);
        assert_eq!(max_cols, 3);

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
            ],
        });

        let (max_rows, max_cols) = TableVars::calculate_dimensions(&t.tbody_rows);
        assert_eq!(max_rows, 4);
        assert_eq!(max_cols, 4);
    }

    #[test]
    fn test_tables() {
        let s = r"
T | b | ct | 80
H | 1,1 | 1,1 | 2,1
H | 1,1 | 1,1 | 2,1
B | 1,2 | 2,1 | 2,1
B | 1,1 | 1,1 | 1,1
F | 1,1 | 1,3

T | b | cb | 35
H | 1,1 | 2,1
B | 1,2 | 2,1 | 2,1

T | b | cb | 50
H | 1,1 | 1,1
B | 1,1 | 1,1

T | b | ct | 100
H | 1,3 | 1,1 | 1,1 | 1,1
H | 2,1 | 1,1 | 1,2 | 1,1
B | 1,1 | 1,1 | 1,2 | 1,1 | 1,1
B | 3,1 | 1,1 | 1,1 | 1,2
B | 1,1 | 1,1 | 1,1 | 1,1 | 1,1
F | 1,2 | 1,1 | 1,1 | 1,1 | 1,1


T | b | cb | 120
H | 2,2 | 1,3 | 1,1
H | 1,1 | 1,1 | 1,1 | 1,2 | 1,1
B | 1,1 | 2,1 | 1,1 | 1,1 | 1,1
B | 1,2 | 1,2 | 1,1 | 1,2
B | 1,1 | 1,1 | 1,1 | 1,1 | 1,1
F | 1,3 | 1,1 | 1,2

T | b | nc | 50
H | 1,2 | 1,1 | 1,1
H | 1,1 | 1,2
B | 1,3 | 1,1
B | 1,1 | 1,1 | 1,1
B | 2,1 | 1,1 | 1,1
F | 1,2 | 1,1

T | b | ct | 150
H | 1,4 | 1,1
H | 1,1 | 2,3
B | 1,2 | 1,1 | 2,2
B | 2,1 | 1,2 | 1,1
B | 1,1 | 1,1 | 1,1 | 1,1
F | 1,1 | 1,1 | 1,2 | 1,1
F | 1,3 | 1,1
F | 1,1 | 1,1 | 1,1 | 1,1

T | b | cb | 200
H | 1,5
H | 1,1 | 1,1 | 1,1 | 1,2
B | 2,2 | 1,3
B | 1,1 | 1,1 | 1,1 | 1,1 | 1,1
B | 1,1 | 1,1 | 1,1 | 1,1 | 1,1
B | 1,4 | 1,1
F | 1,2 | 1,1 | 1,1 | 1,1
F | 1,1 | 1,1 | 1,2


T | u | ct | 80
H | 1,1 | 1,1 | 2,1
H | 1,1 | 1,1 | 2,1
B | 1,2 | 2,1 | 2,1
B | 1,1 | 1,1 | 1,1
F | 1,1 | 1,3
";

        let mut cell_idx = 1;

        let mut tables: Vec<(Table, usize)> = vec![];
        let mut current_table_idx: isize = -1;

        let mut lines = s.lines().collect::<VecDeque<&str>>();
        while let Some(line) = lines.pop_front() {
            if line.trim().is_empty() {
                continue;
            }

            let parts = line.split('|').collect::<Vec<&str>>();
            if parts.len() == 0 {
                continue;
            }

            let section = parts[0].trim();
            match section {
                "T" => {
                    cell_idx = 1;

                    let bordered = parts.get(1).unwrap_or(&"b").trim();
                    let caption = parts.get(2).unwrap_or(&"nc").trim();
                    let width = parts.get(3).unwrap_or(&"80").trim().parse::<usize>().unwrap();

                    let mut t = Table::new(false).with_bordered(bordered == "b");
                    if caption == "ct" {
                        t = t.with_caption("Test Caption", TableCaptionPos::TOP);
                    } else if caption == "cb" {
                        t = t.with_caption("Test Caption", TableCaptionPos::BOTTOM);
                    }

                    tables.push((t, width));
                    current_table_idx = (tables.len() - 1) as isize;
                }
                "B" | "H" | "F" => {
                    if current_table_idx == -1 {
                        continue;
                    }

                    let mut cells = vec![];

                    for part in parts {
                        let parts = part.split(',').collect::<Vec<&str>>();
                        if parts.len() != 2 {
                            continue;
                        }
                        let row = parts[0].trim().parse::<usize>().unwrap();
                        let col = parts[1].trim().parse::<usize>().unwrap();

                        cells.push(TableCell {
                            content: format!("{} {}", section, cell_idx),
                            wrapping: false,
                            h_alignment: TableHalign::LEFT,
                            v_alignment: TableValign::TOP,
                            rowspan: row,
                            colspan: col,
                        });
                        cell_idx += 1;
                    }

                    match section {
                        "H" => tables[current_table_idx as usize].0.add_header_row(TableRow {
                            cells,
                        }),
                        "B" => tables[current_table_idx as usize].0.add_body_row(TableRow {
                            cells,
                        }),
                        "F" => tables[current_table_idx as usize].0.add_footer_row(TableRow {
                            cells,
                        }),
                        _ => (),
                    }
                }
                _ => {
                    // Skip unknown line
                }
            }
        }

        for (t, width) in tables.iter() {
            println!("{}", t.render(*width));
        }
    }

}




/*
 Test format for table:

T | 'b'ordered or 'u'nbordered | 'ct' caption top or 'cb' caption bottom or 'nc' no caption | width
'H'eader or 'B'ody or 'F'ooter | rowspan,colspan | ...

T | b | ct | 80
H | 1,1 | 1,1 | 2,1
H | 1,1 | 1,1 | 2,1
B | 1,2 | 2,1 | 2,1
F | 1,1 | 1,1
--
<table rendering>
--

*/