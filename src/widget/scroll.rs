//! 
//! Types and functionality related to the scrolling behaviour of widgets.
//!

use {Element, Scalar};
use color::Color;
use mouse::Mouse;
use position::{Point, Range, Rect};
use theme::Theme;
use utils::map_range;


/// A type for building a scrollbar widget.
#[derive(Copy, Clone, Debug, PartialEq, RustcEncodable, RustcDecodable)]
pub struct Scrolling {
    /// Is there horizontal scrolling.
    pub horizontal: bool,
    /// Is there vertical scrolling.
    pub vertical: bool,
    /// Styling for the Scrolling.
    pub style: Style,
}


/// State related to scrolling.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct State {
    /// vertical scrollbar.
    pub maybe_vertical: Option<Bar>,
    /// Horizontal scrollbar.
    pub maybe_horizontal: Option<Bar>,
    /// The rectangle representing the Visible area used tot calculate the Bar offsets.
    pub visible: Rect,
    /// The width for vertical scrollbars, the height for horizontal scrollbars.
    pub thickness: Scalar,
    /// The color of the scrollbar.
    pub color: Color,
}


/// Style for the Scrolling.
#[derive(Copy, Clone, Debug, PartialEq, RustcEncodable, RustcDecodable)]
pub struct Style {
    /// The width for vertical scrollbars, the height for horizontal scrollbars.
    pub maybe_thickness: Option<Scalar>,
    /// The color of the scrollbar.
    pub maybe_color: Option<Color>,
}


/// The state of a single scrollbar.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Bar {
    /// The current interaction with the Scrollbar.
    pub interaction: Interaction,
    /// The current scroll position as an offset from the top left.
    pub offset: Scalar,
    /// The maximum possible offset for the handle.
    pub max_offset: Scalar,
    /// The total length of the area occupied by child widgets.
    pub total_length: Scalar,
}


/// The current interaction with the 
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Interaction {
    /// No interaction with the Scrollbar.
    Normal,
    /// Part of the scrollbar is highlighted.
    Highlighted(Elem),
    /// Part of the scrollbar is clicked.
    Clicked(Elem),
}


/// The elements that make up a ScrollBar.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Elem {
    /// The draggable part of the bar and the mouse's position.
    Handle(Scalar),
    /// The track along which the bar can be dragged.
    Track,
}


impl Scrolling {
    /// Constructs the default Scrolling.
    pub fn new() -> Scrolling {
        Scrolling {
            vertical: false,
            horizontal: false,
            style: Style::new(),
        }
    }
}


impl Style {

    /// Construct a new default Style.
    pub fn new() -> Style {
        Style {
            maybe_thickness: None,
            maybe_color: None,
        }
    }

    /// Get the thickness of the scrollbar or a default from the theme.
    pub fn thickness(&self, theme: &Theme) -> Scalar {
        const DEFAULT_THICKNESS: Scalar = 10.0;
        self.maybe_thickness.or(theme.maybe_scrollbar.as_ref().map(|style| {
            style.maybe_thickness.unwrap_or(DEFAULT_THICKNESS)
        })).unwrap_or(DEFAULT_THICKNESS)
    }

    /// Get the Color for an Element.
    pub fn color(&self, theme: &Theme) -> Color {
        self.maybe_color.or(theme.maybe_scrollbar.as_ref().map(|style| {
            style.maybe_color.unwrap_or(theme.shape_color.plain_contrast())
        })).unwrap_or(theme.shape_color.plain_contrast())
    }

}


impl Interaction {
    /// The stateful version of the given color.
    fn color(&self, color: Color) -> Color {
        match *self {
            Interaction::Normal => color,
            Interaction::Highlighted(_) => color.highlighted(),
            Interaction::Clicked(_) => color.clicked(),
        }
    }
}


impl State {

    /// Construct a new State.
    /// The `visible` rect corresponds to a Widget's `kid_area` aka the viewable container.
    /// The `kids` rect is the area *actually occupied* by the children widgets.
    pub fn new(scrolling: Scrolling, 
               visible: Rect,
               kids: Rect,
               theme: &Theme,
               maybe_prev: Option<&State>) -> State
    {

        // The amount required to offset the kids_bounds to their non-scrolled position.
        let x_offset_to_origin = maybe_prev
            .and_then(|prev| prev.maybe_horizontal.map(|bar| bar.pos_offset(visible.w())))
            .unwrap_or(0.0);
        println!("\tx_offset_to_origin: {:?}", x_offset_to_origin);

        // // The non_scrolled start position of the kids bounds.
        // let kids_origin_x_start = kids_bounds.x.start - x_offset_to_origin;
        // println!("\tkids_origin_x_start: {:?}", x_offset_to_origin);

        // // The amount we should use to offset the kids_bounds.
        // let x_offset = ::utils::partial_max(0.0, kids_origin_x_start - kid_area.rect.x.start);
        // println!("\tx_offset: {:?}", x_offset);

        // // The shifted kids bounds.
        // let kids_bounds = if x_offset_to_origin > 0.0 {
        //     kids_bounds.shift_x(kids_origin_x_start)
        // } else {
        //     kids_bounds
        // };
        // println!("\tshifted kids_bounds: {:?}", kids_bounds);



        State {
            maybe_vertical: if scrolling.vertical {
                let maybe_prev = maybe_prev.as_ref()
                    .and_then(|prev| prev.maybe_vertical.as_ref());
                // For a vertical scrollbar, we want the range to start at the top and end at
                // the bottom. To do this, we will use the invert of our visible and kids y ranges.
                Some(Bar::new(visible.y.invert(), kids.y.invert(), maybe_prev))
            } else {
                None
            },
            maybe_horizontal: if scrolling.horizontal {
                let maybe_prev = maybe_prev.as_ref()
                    .and_then(|prev| prev.maybe_horizontal.as_ref());
                Some(Bar::new(visible.x, kids.x, maybe_prev))
            } else {
                None
            },
            visible: visible,
            thickness: scrolling.style.thickness(theme),
            color: scrolling.style.color(theme),
        }
    }

    /// Given some mouse input, update the State and return the resulting State.
    pub fn handle_input(self, mouse: Mouse) -> State {
        use self::Elem::{Handle, Track};
        use self::Interaction::{Normal, Highlighted, Clicked};
        use utils::clamp;

        // Whether or not the mouse is currently over the Bar, and if so, which Elem.
        let is_over_elem = |track: Rect, handle: Rect, mouse_scalar: Scalar| {
            if handle.is_over(mouse.xy) {
                Some(Handle(mouse_scalar))
            } else if track.is_over(mouse.xy) {
                Some(Track)
            } else {
                None
            }
        };

        // Determine the new current `Interaction` for a Bar.
        // The given mouse_scalar is the position of the mouse to be recorded by the Handle.
        // For vertical handle this is mouse.y, for horizontal this is mouse.x.
        let new_interaction = |bar: &Bar, is_over_elem: Option<Elem>, mouse_scalar: Scalar| {
            // If there's no need for a scroll bar, leave the interaction as `Normal`.
            if bar.max_offset == 0.0 {
                Normal
            } else {
                use mouse::ButtonPosition::{Down, Up};
                match (is_over_elem, bar.interaction, mouse.left.position) {
                    (Some(_),    Normal,             Down) => Normal,
                    (Some(elem), _,                  Up)   => Highlighted(elem),
                    (Some(_),    Highlighted(_),     Down) |
                    (_,          Clicked(Handle(_)), Down) => Clicked(Handle(mouse_scalar)),
                    (_,          Clicked(elem),      Down) => Clicked(elem),
                    _                                      => Normal,
                }
            }
        };

        // A function for shifting some current offset by some amount while ensuring it remains
        // within the Bar's Range.
        fn scroll_offset(offset: Scalar, max_offset: Scalar, amount: Scalar) -> Scalar {
            let target_offset = offset + amount;
            // If the offset is before the start, only let it be dragged towards the end.
            let clamp_current_to_max = || clamp(target_offset, offset, max_offset);
            // If the offset is past the end, only let it be dragged towards the start.
            let clamp_zero_to_current = || clamp(target_offset, 0.0, offset);
            // Otherwise, clamp it between 0.0 and the max.
            let clamp_zero_to_max = || clamp(target_offset, 0.0, max_offset);

            // For a positive range, check the start and end of the range normally.
            if max_offset >= 0.0 {
                if      offset < 0.0        { clamp_current_to_max() }
                else if offset > max_offset { clamp_zero_to_current() }
                else                        { clamp_zero_to_max() }

            // Otherwise, check the inverse.
            } else {
                if      offset > 0.0        { clamp_current_to_max() }
                else if offset < max_offset { clamp_zero_to_current() }
                else                        { clamp_zero_to_max() }
            }
        }


        State {

            maybe_vertical: self.maybe_vertical.map(|bar| {
                let track = vertical_track(self.visible, self.thickness);
                let handle = vertical_handle(track, bar.offset, bar.max_offset);

                // Determine whether or not the mouse is over part of the Scrollbar.
                let is_over_elem = is_over_elem(track, handle, mouse.xy[1]);

                // Determine the new current `Interaction`.
                let new_interaction = new_interaction(&bar, is_over_elem, mouse.xy[1]);

                // Determine the new offset for the scrollbar.
                let new_offset = match (bar.interaction, new_interaction) {

                    // When the track is clicked and the handle snaps to the cursor.
                    (Highlighted(Track), Clicked(Handle(mouse_y))) => {
                        // Should try snap the handle so that the mouse is in the middle of it.
                        let target_offset = -((mouse_y - track.top()) + handle.h() / 2.0);
                        clamp(target_offset, 0.0, bar.max_offset)
                    },

                    // When the handle is dragged.
                    (Clicked(Handle(prev_mouse_y)), Clicked(Handle(mouse_y))) =>
                        scroll_offset(bar.offset, bar.max_offset, prev_mouse_y - mouse_y),

                    // The mouse has been scrolled using a wheel/trackpad/touchpad.
                    (_, _) if mouse.scroll.y != 0.0 =>
                        scroll_offset(bar.offset, bar.max_offset, mouse.scroll.y),

                    // Otherwise, we'll assume the offset is unchanged.
                    _ => bar.offset,
                };

                Bar { interaction: new_interaction, offset: new_offset, ..bar }
            }),

            maybe_horizontal: self.maybe_horizontal.map(|bar| {
                let track = horizontal_track(self.visible, self.thickness);
                let handle = horizontal_handle(track, bar.offset, bar.max_offset);

                // Determine whether or not the mouse is over part of the Scrollbar.
                let is_over_elem = is_over_elem(track, handle, mouse.xy[0]);

                // Determine the new current `Interaction`.
                let new_interaction = new_interaction(&bar, is_over_elem, mouse.xy[0]);

                // Determine the new offset for the scrollbar.
                let new_offset = match (bar.interaction, new_interaction) {

                    // When the track is clicked and the handle snaps to the cursor.
                    (Highlighted(Track), Clicked(Handle(mouse_x))) => {
                        // Should try snap the handle so that the mouse is in the middle of it.
                        let target_offset = (mouse_x - track.left()) - handle.w() / 2.0;
                        clamp(target_offset, 0.0, bar.max_offset)
                    },

                    // When the handle is dragged.
                    (Clicked(Handle(prev_mouse_x)), Clicked(Handle(mouse_x))) =>
                        scroll_offset(bar.offset, bar.max_offset, mouse_x - prev_mouse_x),

                    // The mouse has been scrolled using a wheel/trackpad/touchpad.
                    (_, _) if mouse.scroll.x != 0.0 => {
                        println!("Scroll X: {:?}", -mouse.scroll.x);
                        scroll_offset(bar.offset, bar.max_offset, -mouse.scroll.x)
                    },

                    // Otherwise, we'll assume the offset is unchanged.
                    _ => bar.offset,
                };

                Bar { interaction: new_interaction, offset: new_offset, ..bar }
            }),

            .. self
        }
    }

}


// /// The new state of the given scrollbar if it has changed.
// pub fn update(container: Rect, state: &State, maybe_mouse: Option<Mouse>) -> Option<State> {
//     use self::Elem::{Handle, Track};
//     use self::Interaction::{Normal, Highlighted, Clicked};
//     use utils::clamp;
// 
//     // Gives the updated vertical `Bar` if it has changed.
//     let vertical = |bar: &Bar| -> Option<Bar> {
//         // Check to see if the bar has changed and return a new bar if it has.
//         maybe_new_bar(bar, new_interaction, new_offset)
//     };
// 
//     // Gives the updated horizontal `Bar` if it has changed.
//     let horizontal = |bar: &Bar| -> Option<Bar> {
// 
//         // Check to see if the bar has changed and return a new bar if it has.
//         maybe_new_bar(bar, new_interaction, new_offset)
//     };
// 
//     // Produce a new scroll state if there has been any changes in either bar.
//     match (&state.maybe_vertical, &state.maybe_horizontal) {
// 
//         // We have both vertical and horizontal bars.
//         (&Some(ref v_bar), &Some(ref h_bar)) => match (vertical(v_bar), horizontal(h_bar)) {
//             (None, None) => None,
//             (Some(new_v_bar), None) => Some(State { maybe_vertical: Some(new_v_bar), ..*state }),
//             (None, Some(new_h_bar)) => Some(State { maybe_horizontal: Some(new_h_bar), ..*state }),
//             (Some(new_v_bar), Some(new_h_bar)) =>
//                 Some(State {
//                     maybe_vertical: Some(new_v_bar),
//                     maybe_horizontal: Some(new_h_bar),
//                     ..*state
//                 }),
//         },
// 
//         // We only have a vertical scrollbar.
//         (&Some(ref v_bar), &None) => vertical(v_bar).map(|new_v_bar| {
//             State { maybe_vertical: Some(new_v_bar), ..*state }
//         }),
// 
//         // We only have a horizontal scrollbar.
//         (&None, &Some(ref h_bar)) => horizontal(h_bar).map(|new_h_bar| {
//             State { maybe_horizontal: Some(new_h_bar), ..*state }
//         }),
// 
//         // We don't have any scrollbars.
//         (&None, &None) => None,
//     }
// }


/// Construct a renderable Element from the state for the given widget's kid area.
pub fn element(container: Rect, state: State) -> Element {
    use elmesque::element::{empty, layers};
    use elmesque::form::{collage, rect};

    // Get the color via the current interaction.
    let color = state.color;
    let track_color = color.alpha(0.2);
    let thickness = state.thickness;

    // The element for a vertical slider.
    let vertical = |bar: Bar| -> Element {
        // We only want to see the scrollbar if it's highlighted or clicked.
        if let Interaction::Normal = bar.interaction {
            return empty();
        }
        let color = bar.interaction.color(color);
        let track = vertical_track(container, thickness);
        let handle = vertical_handle(track, bar.offset, bar.max_offset);
        let track_form = rect(track.w(), track.h()).filled(track_color)
            .shift(track.x(), track.y());
        let handle_form = rect(handle.w(), handle.h()).filled(color)
            .shift(handle.x(), handle.y());
        collage(container.w() as i32, container.h() as i32, vec![track_form, handle_form])
    };

    // An element for a horizontal slider.
    let horizontal = |bar: Bar| -> Element {
        // We only want to see the scrollbar if it's highlighted or clicked.
        if let Interaction::Normal = bar.interaction {
            return empty();
        }
        let color = bar.interaction.color(color);
        let track = horizontal_track(container, thickness);
        let handle = horizontal_handle(track, bar.offset, bar.max_offset);
        let track_form = rect(track.w(), track.h()).filled(track_color)
            .shift(track.x(), track.y());
        let handle_form = rect(handle.w(), handle.h()).filled(color)
            .shift(handle.x(), handle.y());
        collage(container.w() as i32, container.h() as i32, vec![track_form, handle_form])
    };

    // Whether we draw horizontal or vertical or both depends on our state.
    match (state.maybe_vertical, state.maybe_horizontal) {
        (Some(v_bar), Some(h_bar)) => layers(vec![horizontal(h_bar), vertical(v_bar)]),
        (Some(bar), None) => vertical(bar),
        (None, Some(bar)) => horizontal(bar),
        (None, None) => empty(),
    }
}


impl Bar {

    /// Construct a new Bar with an absolute offset from a visible range and the total range that
    /// is to be scrolled. If there is some previous Bar state, that is also to be considered.
    pub fn new(visible: Range, kids: Range, maybe_prev: Option<&Bar>) -> Bar {

        let total = visible.max_directed(kids);

        let visible_len = visible.magnitude();
        let kids_len = kids.magnitude();
        let total_len = total.magnitude();
        //let scrollable_len = total_len - visible_len;
        let scrollable_len = kids_len - visible_len;

        println!("\tvisible: {:?}", visible);
        println!("\tkids: {:?}", kids);
        println!("\ttotal: {:?}", total);
        println!("\tvisible_len: {:?}", visible_len);
        println!("\tkids_len: {:?}", kids_len);
        println!("\ttotal_len: {:?}", total_len);
        println!("\tscrollable_len: {:?}", scrollable_len);

        // We only need to calculate offsets if we actually have some scrollable area.
        if scrollable_len.is_normal() && scrollable_len.signum() == kids_len.signum() {
            // The start and end differences, so that if both visible points are within the kids
            // range, they will have the same signum as the kids range.
            let start_diff = visible.start - kids.start;
            //let end_diff = kids.end - visible.end;

            let bar_len = (visible_len / kids_len) * visible_len;
            let max_offset = visible_len - bar_len;
            let offset = maybe_prev.map(|bar| bar.offset)
                .unwrap_or_else(|| map_range(start_diff, 0.0, scrollable_len, 0.0, max_offset));
            //let offset = map_range(start_diff, 0.0, scrollable_len, 0.0, max_offset);
            //let offset = map_range(end_diff, total_len - scrollable_len, total_len, 0.0, max_offset);
            let interaction = maybe_prev.map(|bar| bar.interaction).unwrap_or(Interaction::Normal);

            println!("\tbar_len: {:?}", bar_len);
            println!("\tmax_offset: {:?}", max_offset);
            println!("\toffset: {:?}", offset);

            Bar {
                interaction: interaction,
                offset: offset,
                max_offset: max_offset,
                total_length: kids_len,
            }
        // Otherwise our offsets are zeroed.
        } else {
            Bar {
                interaction: Interaction::Normal,
                offset: 0.0,
                max_offset: 0.0,
                total_length: total_len,
            }
        }
    }

    /// Converts the Bar's current offset to a positional offset given some visible range.
    pub fn pos_offset(&self, visible_len: Scalar) -> Scalar {
        if self.max_offset == 0.0
        || self.max_offset > 0.0 && self.offset <= 0.0
        || self.max_offset < 0.0 && self.offset >= 0.0 {
            0.0
        } else {
            let scrollable_len = (self.total_length.abs() - visible_len.abs())
                * self.max_offset.signum();
            -map_range(self.offset, 0.0, self.max_offset, 0.0, scrollable_len)
            // let min_offset = ::utils::partial_min(self.offset, 0.0);
            // let max_offset = ::utils::partial_max(self.offset, self.max_offset);
            // -map_range(self.offset, min_offset, max_offset, 0.0, scrollable_len)
        }
    }

    /// Convert some scalar within the visible_len to a bar offset amount.
    pub fn pos_offset_to_bar_offset(&self, scalar: Scalar, visible_len: Scalar) -> Scalar {
        let scrollable_len = (self.total_length.abs() - visible_len.abs())
            * self.total_length.signum();
        map_range(scalar, 0.0, scrollable_len, 0.0, self.max_offset)
    }

}


/// The area for a vertical scrollbar track as its dimensions and position.
fn vertical_track(container: Rect, thickness: Scalar) -> Rect {
    let w = thickness;
    let x = container.x() + container.w() / 2.0 - w / 2.0;
    Rect {
        x: Range::from_pos_and_len(x, w),
        y: container.y,
    }
}

/// The area for a vertical scrollbar handle as its dimensions and position.
fn vertical_handle(track: Rect, offset: Scalar, max_offset: Scalar) -> Rect {
    let h = track.h() - max_offset;
    let y = track.top() - offset - (h / 2.0);
    Rect {
        x: track.x,
        y: Range::from_pos_and_len(y, h),
    }
}

/// The area for a horizontal scrollbar track as its dimensions and position.
fn horizontal_track(container: Rect, thickness: Scalar) -> Rect {
    let h = thickness;
    let y = container.y() - container.h() / 2.0 + h / 2.0;
    Rect {
        x: container.x,
        y: Range::from_pos_and_len(y, h),
    }
}

/// The area for a horizontal scrollbar handle as its dimensions and position.
fn horizontal_handle(track: Rect, offset: Scalar, max_offset: Scalar) -> Rect {
    let w = track.w() - max_offset;
    let x = track.left() + offset + (w / 2.0);
    Rect {
        x: Range::from_pos_and_len(x, w),
        y: track.y,
    }
}


/// Is the given xy over the area of a scrollbar with the given state.
pub fn is_over(state: &State, container: Rect, target_xy: Point) -> bool {
    if state.maybe_vertical.is_some() {
        return vertical_track(container, state.thickness).is_over(target_xy);
    } else if state.maybe_horizontal.is_some() {
        return horizontal_track(container, state.thickness).is_over(target_xy);
    }
    false
}


/// Whether or not the scrollbar should capture the mouse given previous and new states.
pub fn capture_mouse(prev: &State, new: &State) -> bool {
    match (prev.maybe_vertical, new.maybe_vertical) {
        (Some(ref prev_bar), Some(ref new_bar)) =>
            match (prev_bar.interaction, new_bar.interaction) {
                (Interaction::Highlighted(_), Interaction::Clicked(_)) => return true,
                _ => (),
            },
        _ => (),
    }
    match (prev.maybe_horizontal, new.maybe_horizontal) {
        (Some(ref prev_bar), Some(ref new_bar)) =>
            match (prev_bar.interaction, new_bar.interaction) {
                (Interaction::Highlighted(_), Interaction::Clicked(_)) => return true,
                _ => (),
            },
        _ => (),
    }
    false
}


/// Whether or not the scrollbar should uncapture the mouse given previous and new states.
pub fn uncapture_mouse(prev: &State, new: &State) -> bool {
    match (prev.maybe_vertical, new.maybe_vertical) {
        (Some(ref prev_bar), Some(ref new_bar)) =>
            match (prev_bar.interaction, new_bar.interaction) {
                (Interaction::Clicked(_), Interaction::Highlighted(_)) |
                (Interaction::Clicked(_), Interaction::Normal)         => return true,
                _ => (),
            },
        _ => (),
    }
    match (prev.maybe_horizontal, new.maybe_horizontal) {
        (Some(ref prev_bar), Some(ref new_bar)) =>
            match (prev_bar.interaction, new_bar.interaction) {
                (Interaction::Clicked(_), Interaction::Highlighted(_)) |
                (Interaction::Clicked(_), Interaction::Normal)         => return true,
                _ => (),
            },
        _ => (),
    }
    false
}

