use super::{mark_first, truncate_string};
use std::{borrow::Cow, fmt::Write};

type CalcWidth<T> = Box<dyn Fn(&T) -> usize>;
type CalcValue<T> = Box<dyn Fn(&T) -> Cow<str>>;

pub struct ColumnBuilder<T> {
    name: &'static str,
    calc_width: Option<CalcWidth<T>>,
    calc_value: CalcValue<T>,
    max_width: Option<usize>,
    h_padding: Option<usize>,
    can_shrink: bool,
}

impl<T> ColumnBuilder<T> {
    pub fn new(name: &'static str, calc_value: CalcValue<T>) -> Self {
        Self {
            name,
            calc_width: None,
            calc_value,
            max_width: None,
            h_padding: None,
            can_shrink: false,
        }
    }

    pub fn calc_width(self, calc_width: CalcWidth<T>) -> Self {
        Self {
            calc_width: Some(calc_width),
            ..self
        }
    }

    // pub fn max_width(self, max_width: Option<usize>) -> Self {
    //     Self { max_width, ..self }
    // }

    pub fn h_padding(self, h_padding: Option<usize>) -> Self {
        Self { h_padding, ..self }
    }

    pub fn can_shrink(self, can_shrink: bool) -> Self {
        Self { can_shrink, ..self }
    }

    pub fn build(self) -> Column<T> {
        Column {
            name: self.name,
            calc_width: self.calc_width,
            calc_value: self.calc_value,
            width: self.name.chars().count(),
            max_width: self.max_width,
            h_padding: self.h_padding,
            can_shrink: self.can_shrink,
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
    can_shrink: bool,
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
        max_width: Option<usize>,
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
                        (column.calc_value)(row).chars().count()
                    });
            }
        }
        for column in columns.iter_mut() {
            if let Some(max_width) = column.max_width {
                column.width = column.width.min(max_width);
            }
        }

        if let Some(max_width) = max_width {
            let total_width = columns
                .iter()
                .map(|column| column.width + 2 * column.h_padding.unwrap_or(self.h_padding))
                .sum::<usize>()
                + (columns.len() + 1);
            if let Some(excess_width) = total_width.checked_sub(max_width) {
                let shrinkable_width = columns
                    .iter()
                    .filter(|c| c.can_shrink)
                    .map(|column| column.width)
                    .sum::<usize>();
                let non_shrinkable_width = total_width - shrinkable_width;

                if shrinkable_width != 0 {
                    let shrinkable_columns = columns.iter().filter(|c| c.can_shrink).count();
                    for column in columns.iter_mut().filter(|c| c.can_shrink) {
                        let scaled = column.width
                            - (excess_width * column.width + shrinkable_width - 1)
                                / shrinkable_width;
                        let equal = (max_width - non_shrinkable_width) / shrinkable_columns;
                        column.width = equal.wrapping_add_signed(
                            (scaled as isize - equal as isize)
                                * (3 * max_width + total_width) as isize
                                / (4 * total_width) as isize,
                        );
                    }
                }
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
                truncate_string(&mut value, column.width);
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
