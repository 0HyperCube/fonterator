// fonterator
//
// Copyright (c) 2018-2020 Jeron Aldaron Lau
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// https://apache.org/licenses/LICENSE-2.0>, or the Zlib License, <LICENSE-ZLIB
// or http://opensource.org/licenses/Zlib>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use kurbo::{PathEl, Point};
use rustybuzz::{
    Face as FaceShaper, GlyphBuffer, GlyphInfo, GlyphPosition, UnicodeBuffer,
};
use ttf_parser::{Face, GlyphId, OutlineBuilder};



struct LangFont<'a>(Face<'a>, FaceShaper<'a>);

struct Outliner<'a> {
    // Path to write out to.
    path: &'a mut Vec<PathEl>,
    // How tall the font is (used to invert the Y axis).
    ascender: f32,
    // Translated X and Y positions.
    offset: (f32, f32),
    // Font scaling.
    scale: f32,
}

impl OutlineBuilder for Outliner<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let x = x + self.offset.0;
        let y = self.ascender - (y + self.offset.1);
        self.path.push(PathEl::MoveTo(Point::new(
            (x * self.scale) as f64,
            (y * self.scale) as f64,
        )));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let x = x + self.offset.0;
        let y = self.ascender - (y + self.offset.1);
        self.path.push(PathEl::LineTo(Point::new(
            (x * self.scale) as f64,
            (y * self.scale) as f64,
        )));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let x = x + self.offset.0;
        let x1 = x1 + self.offset.0;
        let y = self.ascender - (y + self.offset.1);
        let y1 = self.ascender - (y1 + self.offset.1);
        self.path.push(PathEl::QuadTo(
            Point::new((x1 * self.scale) as f64, (y1 * self.scale) as f64),
            Point::new((x * self.scale) as f64, (y * self.scale) as f64),
        ));
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let x = x + self.offset.0;
        let x1 = x1 + self.offset.0;
        let x2 = x2 + self.offset.0;
        let y = self.ascender - (y + self.offset.1);
        let y1 = self.ascender - (y1 + self.offset.1);
        let y2 = self.ascender - (y2 + self.offset.1);

        self.path.push(PathEl::CurveTo(
            Point::new((x1 * self.scale) as f64, (y1 * self.scale) as f64),
            Point::new((x2 * self.scale) as f64, (y2 * self.scale) as f64),
            Point::new((x * self.scale) as f64, (y * self.scale) as f64),
        ));
    }

    fn close(&mut self) {
        self.path.push(PathEl::ClosePath);
    }
}

struct StyledFont<'a> {
    // Buffer associated with this font.
    glyph_buffer: Option<GlyphBuffer>,
    // Required
    none: LangFont<'a>,
}

impl StyledFont<'_> {
    fn path(
        &self,
        index: usize,
        path: &mut Vec<PathEl>,
        offset: &mut (i32, i32),
    ) {
        
        let GlyphPosition {
            x_advance,
            y_advance,
            x_offset,
            y_offset,
            ..
        } = self.glyph_buffer.as_ref().unwrap().glyph_positions()[index];
        let GlyphInfo {
            glyph_id,
            cluster: _,
            ..
        } = self.glyph_buffer.as_ref().unwrap().glyph_infos()[index];
        let glyph_id = GlyphId(glyph_id as u16);
        let scale = (self.none.0.height() as f32).recip();

        // let xy = (xy.0 + x_offset as f32 * scale, -xy.1 - y_offset as f32 * scale);
        let ascender = self.none.0.ascender() as f32 * scale;
        let x_offset = x_offset + offset.0;
        let y_offset = y_offset + offset.1;
        offset.0 += x_advance;
        offset.1 += y_advance;
        let offset = (
            x_offset as f32,
            (y_offset - self.none.0.ascender() as i32) as f32,
        );

        self.none.0.outline_glyph(
            glyph_id,
            &mut Outliner {
                path,
                ascender,
                scale,
                offset,
            },
        );
    }
}

/// A collection of TTF/OTF fonts used as a single font.
#[allow(missing_debug_implementations)]
#[derive(Default)]
pub struct Font<'a> {
    paths: Vec<PathEl>,
    fonts: Vec<StyledFont<'a>>,
}

impl<'a> Font<'a> {
    /// Create a new `Font`.  Add glyphs with `push()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a TTF or OTF font's glyphs to this `Font`.
    pub fn push<B: Into<&'a [u8]>>(mut self, font_data: B) -> Option<Self> {
        let font_data = font_data.into();
        let face = (
            Face::from_slice(font_data, 0).ok()?,
            FaceShaper::from_slice(font_data, 0)?,
        );
        let none = LangFont(face.0, face.1);

        self.fonts.push(StyledFont {
            none,
            glyph_buffer: None,
        });
        Some(self)
    }

    /// Render some text.  Returns an iterator.
    ///  - `text`: text to render.
    ///  - `row_length`: x position for line wrapping
    ///  - `row_drop`: y shift for new lines
    ///
    ///  Returns an iterator which generates the path from characters (see
    ///  [`TextPathIterator`]) and a number indicating how many characters are
    ///  leftover (not rendered).
    pub fn render<'b>(
        &'b mut self,
        text: &str,
        row_length: i32,
        row_drop: i32,
    ) -> TextPathIterator<'a, 'b> {

        // Replace glyph buffer using text.
        // FIXME: Currently only using first font.
        self.fonts[0].glyph_buffer = Some({
            let mut unicode_buffer =
                if let Some(buf) = self.fonts[0].glyph_buffer.take() {
                    buf.clear()
                } else {
                    UnicodeBuffer::new()
                };
            unicode_buffer.push_str(text);
            rustybuzz::shape(&self.fonts[0].none.1, &[], unicode_buffer)
        });

        // Pass over glyphs, looking for where to stop.
        let positions = self.fonts[0]
            .glyph_buffer
            .as_ref()
            .unwrap()
            .glyph_positions();
        let until = positions.len();


        // Handle line breaks

        // How far line moved on x
        let mut x_pos = 0;
        let mut line_break_indicies = Vec::new();
        let mut last_space_index = None;
        for ((index, character), glyph) in text.char_indices().zip(positions){
            x_pos+= glyph.x_advance;
            if character == ' '{
                println!("Space at {}", index);
                last_space_index = Some(index);
            }
            else if character == '\n'{
                line_break_indicies.push(index);
                x_pos = 0;
                last_space_index = None;
            }
            else if x_pos > row_length{
                if let Some(x) = last_space_index{
                    line_break_indicies.push(x);
                }else{
                    line_break_indicies.push(index);
                }
                
                x_pos = 0;
                last_space_index = None;
            }
        }

        // Return iterator over PathOps
            TextPathIterator {
                fontc: self,
                until,
                index: 0,
                path_i: 0,
                offset: (0, 0),
                line_break_indicies,
                row_drop
            }

    }
}

/// Iterator that generates a path from characters.
#[allow(missing_debug_implementations)]
pub struct TextPathIterator<'a, 'b> {
    // Contains reusable glyph and path buffers.
    fontc: &'b mut Font<'a>,
    // Index to stop rendering at.
    until: usize,
    // Current glyph index.
    index: usize,
    // Index for `PathEl`s.
    path_i: usize,
    /// The x and y offset.
    pub offset: (i32, i32),
    // Letter indecies where there are line breaks
    line_break_indicies: Vec<usize>,
    // Y offset change on line breaks
    row_drop: i32,
}

impl Iterator for TextPathIterator<'_, '_> {
    type Item = PathEl;

    fn next(&mut self) -> Option<PathEl> {
        // First, check for remaining PathEl's in the glyph path buffer.
        if self.path_i != self.fontc.paths.len() {
            let path_op = self.fontc.paths[self.path_i];
            self.path_i += 1;
            return Some(path_op);
        }
        // Because no path ops were left, clear buffer for reuse.
        self.fontc.paths.clear();
        self.path_i = 0;
        // Check for remaining glyphs in the GlyphBuffer.
        if self.index != self.until {
            self.fontc.fonts[0].path(
                self.index,
                &mut self.fontc.paths,
                &mut self.offset,
            );
            println!("off {:?}", self.offset);
            if self.line_break_indicies.contains(&self.index){
                self.offset.0 = 0;
                self.offset.1 += self.row_drop;
            }
            self.index += 1;
            self.next()
        } else {
            None
        }
    }
}

/// Gets the source sans pro font
pub fn source_font() -> Font<'static> {
    const SOURCE_FONT: &[u8] = include_bytes!("sourcesanspro/SourceSansPro-Regular.ttf");
    Font::new()
        .push(SOURCE_FONT)
        .unwrap()
}

#[test]
fn t(){
    let mut f = source_font();
    let iter = f.render(
        "In publishing and graphic design, Lorem ipsum is a placeholder text commonly used to demonstrate the visual form of a document or a typeface without relying on meaningful content.",
        10000,
        -800
    );
    println!("{:?}", iter.offset);
    let _ = kurbo::BezPath::from_vec(iter.map(|p| p).collect());
    
}
