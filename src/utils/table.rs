use super::mark_first;
use std::{borrow::Cow, fmt::Write};

type CalcWidth<T> = Box<dyn Fn(&T) -> usize>;
type CalcValue<T> = Box<dyn Fn(&T) -> Cow<str>>;

pub struct ColumnBuilder<T> {
    name: &'static str,
    calc_width: Option<CalcWidth<T>>,
    calc_value: CalcValue<T>,
    max_width: Option<usize>,
    h_padding: Option<usize>,
}

impl<T> ColumnBuilder<T> {
    pub fn new(name: &'static str, calc_value: CalcValue<T>) -> Self {
        Self {
            name,
            calc_width: None,
            calc_value,
            max_width: None,
            h_padding: None,
        }
    }

    pub fn calc_width(self, calc_width: CalcWidth<T>) -> Self {
        Self {
            calc_width: Some(calc_width),
            ..self
        }
    }

    pub fn max_width(self, max_width: Option<usize>) -> Self {
        Self { max_width, ..self }
    }

    pub fn h_padding(self, h_padding: Option<usize>) -> Self {
        Self { h_padding, ..self }
    }

    pub fn build(self) -> Column<T> {
        Column {
            name: self.name,
            calc_width: self.calc_width,
            calc_value: self.calc_value,
            width: self.name.len(),
            max_width: self.max_width,
            h_padding: self.h_padding,
        }
    }
}

pub struct Column<T> {
    name: &'static str,
    calc_width: Option<CalcWidth<T>>,
    calc_value: CalcValue<T>,
    width: usize,
    max_width: Option<usize>,
    h_padding: Option<usize>,
}

pub struct Builder {
    use_box_drawing: bool,
    h_padding: usize,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            use_box_drawing: true,
            h_padding: 2,
        }
    }
}

impl Builder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn use_box_drawing(self, use_box_drawing: bool) -> Self {
        Self {
            use_box_drawing,
            ..self
        }
    }

    pub fn h_padding(self, h_padding: usize) -> Self {
        Self { h_padding, ..self }
    }

    fn side_h(&self, output: &mut String, width: usize) {
        if self.use_box_drawing {
            let _ = write!(output, "{empty:─<width$}", empty = "");
        } else {
            let _ = write!(output, "{empty:-<width$}", empty = "");
        }
    }

    pub fn build<'a, T: 'a>(
        self,
        columns: &mut [Column<T>],
        data: impl IntoIterator<Item = &'a T> + Clone,
    ) -> String {
        let (corners, border_v) = if self.use_box_drawing {
            (['┌', '┬', '┐', '├', '┼', '┤', '└', '┴', '┘'], '│')
        } else {
            (['|'; 9], '|')
        };

        for row in data.clone() {
            for column in columns.iter_mut() {
                column.width = column
                    .width
                    .max(if let Some(calc_width) = &column.calc_width {
                        calc_width(row)
                    } else {
                        (column.calc_value)(row).len()
                    });
            }
        }
        for column in columns.iter_mut() {
            if let Some(max_width) = column.max_width {
                column.width = column.width.min(max_width);
            }
        }

        let mut output = String::new();

        output.push(corners[0]);
        for (is_first, column) in mark_first(columns.iter()) {
            if !is_first {
                output.push(corners[1]);
            }
            self.side_h(
                &mut output,
                column.width + 2 * column.h_padding.unwrap_or(self.h_padding),
            );
        }
        output.push(corners[2]);
        output.push('\n');

        output.push(border_v);
        for (is_first, column) in mark_first(columns.iter()) {
            if !is_first {
                output.push(border_v);
            }
            let _ = write!(
                output,
                "{empty: <h_padding$}{name:^width$}{empty: <h_padding$}",
                empty = "",
                h_padding = column.h_padding.unwrap_or(self.h_padding),
                name = column.name,
                width = column.width,
            );
        }
        output.push(border_v);
        output.push('\n');

        output.push(corners[3]);
        for (is_first, column) in mark_first(columns.iter()) {
            if !is_first {
                output.push(corners[4]);
            }
            self.side_h(
                &mut output,
                column.width + 2 * column.h_padding.unwrap_or(self.h_padding),
            );
        }
        output.push(corners[5]);
        output.push('\n');

        for row in data {
            output.push(border_v);
            for (is_first, column) in mark_first(columns.iter()) {
                if !is_first {
                    output.push(border_v);
                }
                let mut value = (column.calc_value)(row).into_owned();
                if value.len() > column.width {
                    value.truncate(column.width - 1);
                    value.push('…');
                }
                let _ = write!(
                    output,
                    "{empty: <h_padding$}{value:<width$}{empty: <h_padding$}",
                    empty = "",
                    h_padding = column.h_padding.unwrap_or(self.h_padding),
                    width = column.width
                );
            }
            output.push(border_v);
            output.push('\n');
        }

        output.push(corners[6]);
        for (is_first, column) in mark_first(columns.iter()) {
            if !is_first {
                output.push(corners[7]);
            }
            self.side_h(
                &mut output,
                column.width + 2 * column.h_padding.unwrap_or(self.h_padding),
            );
        }
        output.push(corners[8]);
        output
    }
}
