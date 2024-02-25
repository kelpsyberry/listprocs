use super::{mark_first, truncate_string};
use std::{borrow::Cow, fmt::Write};

type CalcWidth<'a, T> = Box<dyn Fn(&T) -> usize + 'a>;
type CalcValue<'a, T> = Box<dyn Fn(&T) -> Cow<str> + 'a>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    BoxDrawing,
    Ascii,
    None,
}

pub struct ColumnBuilder<'a, T> {
    name: &'static str,
    calc_width: Option<CalcWidth<'a, T>>,
    calc_value: CalcValue<'a, T>,
    max_width: Option<usize>,
    h_padding: Option<usize>,
    can_shrink: bool,
}

impl<'a, T> ColumnBuilder<'a, T> {
    pub fn new(name: &'static str, calc_value: CalcValue<'a, T>) -> Self {
        Self {
            name,
            calc_width: None,
            calc_value,
            max_width: None,
            h_padding: None,
            can_shrink: false,
        }
    }

    pub fn calc_width(self, calc_width: CalcWidth<'a, T>) -> Self {
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

    pub fn build(self) -> Column<'a, T> {
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

pub struct Column<'a, T> {
    name: &'static str,
    calc_width: Option<CalcWidth<'a, T>>,
    calc_value: CalcValue<'a, T>,
    width: usize,
    max_width: Option<usize>,
    h_padding: Option<usize>,
    can_shrink: bool,
}

pub struct Builder {
    style: Style,
    h_padding: usize,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            style: Style::BoxDrawing,
            h_padding: 2,
        }
    }
}

impl Builder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn style(self, style: Style) -> Self {
        Self { style, ..self }
    }

    pub fn h_padding(self, h_padding: usize) -> Self {
        Self { h_padding, ..self }
    }

    fn build_plain<'a, T: 'a>(
        self,
        columns: &mut [Column<T>],
        data: impl IntoIterator<Item = &'a T> + Clone,
    ) -> String {
        if let Some((_, columns_before_last)) = columns.split_last_mut() {
            for row in data.clone() {
                for column in columns_before_last.iter_mut() {
                    column.width = column
                        .width
                        .max(if let Some(calc_width) = &column.calc_width {
                            calc_width(row)
                        } else {
                            (column.calc_value)(row).chars().count()
                        });
                }
            }
        }

        let mut output = String::new();

        for column in columns.iter() {
            let _ = write!(
                output,
                " {name:width$}",
                name = column.name,
                width = column.width,
            );
        }
        output.push('\n');

        for row in data {
            for column in columns.iter() {
                let _ = write!(
                    output,
                    " {value:width$}",
                    value = (column.calc_value)(row).into_owned(),
                    width = column.width
                );
            }
            output.push('\n');
        }

        output
    }

    fn build_bordered<'a, T: 'a>(
        self,
        columns: &mut [Column<T>],
        data: impl IntoIterator<Item = &'a T> + Clone,
        max_width: Option<usize>,
        use_box_drawing: bool,
    ) -> String {
        fn border_h(output: &mut String, width: usize, use_box_drawing: bool) {
            let _ = if use_box_drawing {
                write!(output, "{empty:─<width$}", empty = "")
            } else {
                write!(output, "{empty:-<width$}", empty = "")
            };
        }

        let (corners, border_v) = if use_box_drawing {
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
                let mut shrinkable_width = columns
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

                shrinkable_width = columns
                    .iter()
                    .filter(|c| c.can_shrink)
                    .map(|column| column.width)
                    .sum::<usize>();
                for column in columns
                    .iter_mut()
                    .take(max_width - (shrinkable_width + non_shrinkable_width))
                {
                    column.width += 1;
                }
            }
        }

        let mut output = String::new();

        output.push(corners[0]);
        for (is_first, column) in mark_first(columns.iter()) {
            if !is_first {
                output.push(corners[1]);
            }
            border_h(
                &mut output,
                column.width + 2 * column.h_padding.unwrap_or(self.h_padding),
                use_box_drawing,
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
                "{empty:h_padding$}{name:^width$}{empty:h_padding$}",
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
            border_h(
                &mut output,
                column.width + 2 * column.h_padding.unwrap_or(self.h_padding),
                use_box_drawing,
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
                    "{empty:h_padding$}{value:width$}{empty:h_padding$}",
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
            border_h(
                &mut output,
                column.width + 2 * column.h_padding.unwrap_or(self.h_padding),
                use_box_drawing,
            );
        }
        output.push(corners[8]);
        output.push('\n');
        output
    }

    pub fn build<'a, T: 'a>(
        self,
        columns: &mut [Column<T>],
        data: impl IntoIterator<Item = &'a T> + Clone,
        max_width: Option<usize>,
    ) -> String {
        match &self.style {
            Style::BoxDrawing | Style::Ascii => {
                let use_box_drawing = matches!(self.style, Style::BoxDrawing);
                self.build_bordered(columns, data, max_width, use_box_drawing)
            }
            Style::None => self.build_plain(columns, data),
        }
    }
}
