use super::{mark_first, truncate_string};
use std::{borrow::Cow, fmt::Write, marker::PhantomData};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    BoxDrawing,
    Ascii,
    None,
}

pub trait Column<T> {
    fn name(&self) -> &str;
    fn calc_width(&self, value: &T) -> usize;
    fn calc_value<'a>(&self, value: &'a T) -> Cow<'a, str>;
    fn max_width(&self) -> Option<usize>;
    fn h_padding(&self) -> Option<usize>;
    fn can_shrink(&self) -> bool;
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

    pub fn build<T, C: Column<T>>(
        self,
        columns: impl IntoIterator<Item = C>,
    ) -> TableTemplate<T, C> {
        TableTemplate {
            style: self.style,
            columns: columns
                .into_iter()
                .map(|column| ColumnData {
                    h_padding: column.h_padding().unwrap_or(self.h_padding),
                    width: 0,
                    inner: column,
                    _data: PhantomData,
                })
                .collect(),
        }
    }
}

struct ColumnData<T, C: Column<T>> {
    inner: C,
    h_padding: usize,
    width: usize,
    _data: PhantomData<T>,
}

pub struct TableTemplate<T, C: Column<T>> {
    style: Style,
    columns: Vec<ColumnData<T, C>>,
}

impl<T, C: Column<T>> TableTemplate<T, C> {
    fn format_plain<'a>(&mut self, data: impl IntoIterator<Item = &'a T> + Clone) -> String
    where
        T: 'a,
    {
        for column in &mut self.columns {
            column.width = column.inner.name().len();
        }

        if let Some((_, columns_before_last)) = self.columns.split_last_mut() {
            for row in data.clone() {
                for column in columns_before_last.iter_mut() {
                    column.width = column.width.max(column.inner.calc_width(row));
                }
            }
        }

        let mut output = String::new();

        for column in &self.columns {
            let _ = write!(
                output,
                " {name:width$}",
                name = column.inner.name(),
                width = column.width,
            );
        }
        output.push('\n');

        for row in data {
            for column in &self.columns {
                let _ = write!(
                    output,
                    " {value:width$}",
                    value = column.inner.calc_value(row),
                    width = column.width
                );
            }
            output.push('\n');
        }

        output
    }

    fn format_bordered<'a>(
        &mut self,
        data: impl IntoIterator<Item = &'a T> + Clone,
        max_width: Option<usize>,
        use_box_drawing: bool,
    ) -> String
    where
        T: 'a,
    {
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

        for column in &mut self.columns {
            column.width = column.inner.name().len();
        }
        for row in data.clone() {
            for column in &mut self.columns {
                column.width = column.width.max(column.inner.calc_width(row));
            }
        }
        for column in &mut self.columns {
            if let Some(max_width) = column.inner.max_width() {
                column.width = column.width.min(max_width);
            }
        }

        if let Some(max_width) = max_width {
            let total_width = self
                .columns
                .iter()
                .map(|column| column.width + 2 * column.h_padding)
                .sum::<usize>()
                + (self.columns.len() + 1);
            if let Some(excess_width) = total_width.checked_sub(max_width) {
                let mut shrinkable_width = self
                    .columns
                    .iter()
                    .filter(|c| c.inner.can_shrink())
                    .map(|column| column.width)
                    .sum::<usize>();
                let non_shrinkable_width = total_width - shrinkable_width;

                if shrinkable_width != 0 {
                    let shrinkable_columns =
                        self.columns.iter().filter(|c| c.inner.can_shrink()).count();
                    for column in self.columns.iter_mut().filter(|c| c.inner.can_shrink()) {
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

                shrinkable_width = self
                    .columns
                    .iter()
                    .filter(|c| c.inner.can_shrink())
                    .map(|column| column.width)
                    .sum::<usize>();
                for column in self
                    .columns
                    .iter_mut()
                    .take(max_width - (shrinkable_width + non_shrinkable_width))
                {
                    column.width += 1;
                }
            }
        }

        let mut output = String::new();

        output.push(corners[0]);
        for (is_first, column) in mark_first(&self.columns) {
            if !is_first {
                output.push(corners[1]);
            }
            border_h(
                &mut output,
                column.width + 2 * column.h_padding,
                use_box_drawing,
            );
        }
        output.push(corners[2]);
        output.push('\n');

        output.push(border_v);
        for (is_first, column) in mark_first(&self.columns) {
            if !is_first {
                output.push(border_v);
            }
            let _ = write!(
                output,
                "{empty:h_padding$}{name:^width$}{empty:h_padding$}",
                empty = "",
                h_padding = column.h_padding,
                name = column.inner.name(),
                width = column.width,
            );
        }
        output.push(border_v);
        output.push('\n');

        output.push(corners[3]);
        for (is_first, column) in mark_first(&self.columns) {
            if !is_first {
                output.push(corners[4]);
            }
            border_h(
                &mut output,
                column.width + 2 * column.h_padding,
                use_box_drawing,
            );
        }
        output.push(corners[5]);
        output.push('\n');

        for row in data {
            output.push(border_v);
            for (is_first, column) in mark_first(&self.columns) {
                if !is_first {
                    output.push(border_v);
                }
                let mut value = column.inner.calc_value(row).into_owned();
                truncate_string(&mut value, column.width);
                let _ = write!(
                    output,
                    "{empty:h_padding$}{value:width$}{empty:h_padding$}",
                    empty = "",
                    h_padding = column.h_padding,
                    width = column.width
                );
            }
            output.push(border_v);
            output.push('\n');
        }

        output.push(corners[6]);
        for (is_first, column) in mark_first(&self.columns) {
            if !is_first {
                output.push(corners[7]);
            }
            border_h(
                &mut output,
                column.width + 2 * column.h_padding,
                use_box_drawing,
            );
        }
        output.push(corners[8]);
        output.push('\n');
        output
    }

    pub fn format<'a>(
        &mut self,
        data: impl IntoIterator<Item = &'a T> + Clone,
        max_width: Option<usize>,
    ) -> String
    where
        T: 'a,
    {
        match &self.style {
            Style::BoxDrawing | Style::Ascii => {
                let use_box_drawing = matches!(self.style, Style::BoxDrawing);
                self.format_bordered(data, max_width, use_box_drawing)
            }
            Style::None => self.format_plain(data),
        }
    }
}
