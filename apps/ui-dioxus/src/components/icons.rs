use dioxus::prelude::*;

#[component]
pub fn IconBrand() -> Element {
    rsx! {
        svg {
            width: "14",
            height: "14",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2.2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M6 4l12 8-12 8V4z" }
            line { x1: "19", y1: "5", x2: "19", y2: "19" }
        }
    }
}

#[component]
pub fn IconMedia() -> Element {
    rsx! {
        svg {
            width: "17",
            height: "17",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.75",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            rect { x: "3", y: "4", width: "18", height: "16", rx: "3" }
            line { x1: "7", y1: "4", x2: "7", y2: "20" }
            line { x1: "17", y1: "4", x2: "17", y2: "20" }
            line { x1: "3", y1: "12", x2: "21", y2: "12" }
        }
    }
}

#[component]
pub fn IconSubtitles() -> Element {
    rsx! {
        svg {
            width: "17",
            height: "17",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.75",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            rect { x: "3", y: "5", width: "18", height: "14", rx: "3" }
            line { x1: "7", y1: "11", x2: "13", y2: "11" }
            line { x1: "16", y1: "11", x2: "17", y2: "11" }
            line { x1: "7", y1: "15", x2: "10", y2: "15" }
            line { x1: "13", y1: "15", x2: "17", y2: "15" }
        }
    }
}

#[component]
pub fn IconShield() -> Element {
    rsx! {
        svg {
            width: "17",
            height: "17",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.75",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M12 3l7 3v5.5c0 4.5-3 8.1-7 9.5-4-1.4-7-5-7-9.5V6l7-3z" }
            path { d: "M9.5 12l1.8 1.8 3.6-3.8" }
        }
    }
}

#[component]
pub fn IconGlossary() -> Element {
    rsx! {
        svg {
            width: "17",
            height: "17",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.75",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H19v20H6.5a2.5 2.5 0 0 1 0-5H19" }
            line { x1: "9", y1: "7", x2: "15", y2: "7" }
            line { x1: "9", y1: "11", x2: "13", y2: "11" }
        }
    }
}

#[component]
pub fn IconExport() -> Element {
    rsx! {
        svg {
            width: "16",
            height: "16",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.8",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M12 3v12" }
            path { d: "M7.5 7.5L12 3l4.5 4.5" }
            path { d: "M4 15v3a3 3 0 0 0 3 3h10a3 3 0 0 0 3-3v-3" }
        }
    }
}

#[component]
pub fn IconKeyboard() -> Element {
    rsx! {
        svg {
            width: "16",
            height: "16",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.75",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            rect { x: "2", y: "6", width: "20", height: "12", rx: "2.5" }
            line { x1: "6", y1: "10", x2: "6.01", y2: "10" }
            line { x1: "10", y1: "10", x2: "10.01", y2: "10" }
            line { x1: "14", y1: "10", x2: "14.01", y2: "10" }
            line { x1: "18", y1: "10", x2: "18.01", y2: "10" }
            line { x1: "8", y1: "14", x2: "16", y2: "14" }
        }
    }
}

#[component]
pub fn IconLock() -> Element {
    rsx! {
        svg {
            width: "12",
            height: "12",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            rect { x: "5", y: "11", width: "14", height: "10", rx: "2.5" }
            path { d: "M8 11V7a4 4 0 0 1 8 0v4" }
        }
    }
}

#[component]
pub fn IconUnlock() -> Element {
    rsx! {
        svg {
            width: "12",
            height: "12",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            rect { x: "5", y: "11", width: "14", height: "10", rx: "2.5" }
            path { d: "M8 11V7a4 4 0 0 1 7.6-1.7" }
        }
    }
}

#[component]
pub fn IconSparkle() -> Element {
    rsx! {
        svg {
            width: "13",
            height: "13",
            view_box: "0 0 24 24",
            fill: "currentColor",
            path { d: "M12 2l2.1 6.3L20.5 10l-6.4 2.1L12 18.5l-2.1-6.4L3.5 10l6.4-1.7L12 2z" }
        }
    }
}

#[component]
pub fn IconChevronDown() -> Element {
    rsx! {
        svg {
            width: "12",
            height: "12",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2.2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M6 9l6 6 6-6" }
        }
    }
}

#[component]
pub fn IconSkipBack() -> Element {
    rsx! {
        svg {
            width: "14",
            height: "14",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            polygon { points: "19 20 9 12 19 4 19 20" }
            line { x1: "5", y1: "19", x2: "5", y2: "5" }
        }
    }
}

#[component]
pub fn IconStepBack() -> Element {
    rsx! {
        svg {
            width: "14",
            height: "14",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M15 18l-6-6 6-6" }
        }
    }
}

#[component]
pub fn IconStepForward() -> Element {
    rsx! {
        svg {
            width: "14",
            height: "14",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M9 18l6-6-6-6" }
        }
    }
}

#[component]
pub fn IconPlay() -> Element {
    rsx! {
        svg {
            width: "13",
            height: "13",
            view_box: "0 0 24 24",
            fill: "currentColor",
            path { d: "M7 4.5v15l12.5-7.5L7 4.5z" }
        }
    }
}

#[component]
pub fn IconPause() -> Element {
    rsx! {
        svg {
            width: "13",
            height: "13",
            view_box: "0 0 24 24",
            fill: "currentColor",
            rect { x: "6", y: "4", width: "4.2", height: "16", rx: "1.4" }
            rect { x: "13.8", y: "4", width: "4.2", height: "16", rx: "1.4" }
        }
    }
}

#[component]
pub fn IconUndo() -> Element {
    rsx! {
        svg {
            width: "14",
            height: "14",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.9",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M9 14L4 9l5-5" }
            path { d: "M4 9h10.5a5.5 5.5 0 0 1 0 11H11" }
        }
    }
}

#[component]
pub fn IconRedo() -> Element {
    rsx! {
        svg {
            width: "14",
            height: "14",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.9",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M15 14l5-5-5-5" }
            path { d: "M20 9H9.5a5.5 5.5 0 0 0 0 11H13" }
        }
    }
}

#[component]
pub fn IconClose() -> Element {
    rsx! {
        svg {
            width: "13",
            height: "13",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2.1",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            line { x1: "18", y1: "6", x2: "6", y2: "18" }
            line { x1: "6", y1: "6", x2: "18", y2: "18" }
        }
    }
}

#[component]
pub fn IconScissors() -> Element {
    rsx! {
        svg {
            width: "12",
            height: "12",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            circle { cx: "6", cy: "6", r: "3" }
            circle { cx: "6", cy: "18", r: "3" }
            line { x1: "20", y1: "4", x2: "8.12", y2: "15.88" }
            line { x1: "14.47", y1: "14.48", x2: "20", y2: "20" }
            line { x1: "8.12", y1: "8.12", x2: "12", y2: "12" }
        }
    }
}

#[component]
pub fn IconMerge() -> Element {
    rsx! {
        svg {
            width: "12",
            height: "12",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M8 6l4-4 4 4" }
            path { d: "M12 2v10.3a4 4 0 0 1-1.17 2.83L4 22" }
            path { d: "M20 22l-5-5" }
        }
    }
}

#[component]
pub fn IconUpload() -> Element {
    rsx! {
        svg {
            width: "22",
            height: "22",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.75",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M12 16V4" }
            path { d: "M7 9l5-5 5 5" }
            path { d: "M4 17v1a3 3 0 0 0 3 3h10a3 3 0 0 0 3-3v-1" }
        }
    }
}

#[component]
pub fn IconEye() -> Element {
    rsx! {
        svg {
            width: "13",
            height: "13",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.9",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z" }
            circle { cx: "12", cy: "12", r: "3" }
        }
    }
}

#[component]
pub fn IconCrosshair() -> Element {
    rsx! {
        svg {
            width: "13",
            height: "13",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.9",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            rect { x: "5", y: "5", width: "14", height: "14", rx: "2.5" }
            rect { x: "9", y: "9", width: "6", height: "6", rx: "1" }
        }
    }
}

#[component]
pub fn IconPlus() -> Element {
    rsx! {
        svg {
            width: "13",
            height: "13",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2.2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            line { x1: "12", y1: "5", x2: "12", y2: "19" }
            line { x1: "5", y1: "12", x2: "19", y2: "12" }
        }
    }
}

#[component]
pub fn IconSun() -> Element {
    rsx! {
        svg {
            width: "15",
            height: "15",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.9",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            circle { cx: "12", cy: "12", r: "4" }
            line { x1: "12", y1: "2.5", x2: "12", y2: "5" }
            line { x1: "12", y1: "19", x2: "12", y2: "21.5" }
            line { x1: "4.93", y1: "4.93", x2: "6.7", y2: "6.7" }
            line { x1: "17.3", y1: "17.3", x2: "19.07", y2: "19.07" }
            line { x1: "2.5", y1: "12", x2: "5", y2: "12" }
            line { x1: "19", y1: "12", x2: "21.5", y2: "12" }
            line { x1: "4.93", y1: "19.07", x2: "6.7", y2: "17.3" }
            line { x1: "17.3", y1: "6.7", x2: "19.07", y2: "4.93" }
        }
    }
}

#[component]
pub fn IconMoon() -> Element {
    rsx! {
        svg {
            width: "14",
            height: "14",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.9",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" }
        }
    }
}

#[component]
pub fn IconZoomIn() -> Element {
    rsx! {
        svg {
            width: "13",
            height: "13",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            circle { cx: "11", cy: "11", r: "7" }
            line { x1: "20", y1: "20", x2: "16", y2: "16" }
            line { x1: "11", y1: "8", x2: "11", y2: "14" }
            line { x1: "8", y1: "11", x2: "14", y2: "11" }
        }
    }
}

#[component]
pub fn IconZoomOut() -> Element {
    rsx! {
        svg {
            width: "13",
            height: "13",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            circle { cx: "11", cy: "11", r: "7" }
            line { x1: "20", y1: "20", x2: "16", y2: "16" }
            line { x1: "8", y1: "11", x2: "14", y2: "11" }
        }
    }
}

#[component]
pub fn IconMagnet() -> Element {
    rsx! {
        svg {
            width: "13",
            height: "13",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.9",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M6 4v8a6 6 0 0 0 12 0V4" }
            line { x1: "6", y1: "8", x2: "10", y2: "8" }
            line { x1: "14", y1: "8", x2: "18", y2: "8" }
        }
    }
}

#[component]
pub fn IconSliders() -> Element {
    rsx! {
        svg {
            width: "13",
            height: "13",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.9",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            line { x1: "4", y1: "7", x2: "20", y2: "7" }
            circle { cx: "9", cy: "7", r: "2" }
            line { x1: "4", y1: "17", x2: "20", y2: "17" }
            circle { cx: "15", cy: "17", r: "2" }
        }
    }
}

#[component]
pub fn IconSearch() -> Element {
    rsx! {
        svg {
            width: "13",
            height: "13",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            circle { cx: "11", cy: "11", r: "7" }
            line { x1: "20", y1: "20", x2: "16", y2: "16" }
        }
    }
}

#[component]
pub fn IconWave() -> Element {
    rsx! {
        svg {
            width: "13",
            height: "13",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            line { x1: "4", y1: "10", x2: "4", y2: "14" }
            line { x1: "8", y1: "6", x2: "8", y2: "18" }
            line { x1: "12", y1: "3", x2: "12", y2: "21" }
            line { x1: "16", y1: "7", x2: "16", y2: "17" }
            line { x1: "20", y1: "10", x2: "20", y2: "14" }
        }
    }
}

#[component]
pub fn IconPlate() -> Element {
    rsx! {
        svg {
            width: "13",
            height: "13",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.9",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            rect { x: "3", y: "7", width: "18", height: "10", rx: "2" }
            line { x1: "7", y1: "12", x2: "17", y2: "12" }
        }
    }
}

#[component]
pub fn IconFace() -> Element {
    rsx! {
        svg {
            width: "13",
            height: "13",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.9",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            circle { cx: "12", cy: "12", r: "8" }
            circle { cx: "9.5", cy: "10", r: "0.8", fill: "currentColor" }
            circle { cx: "14.5", cy: "10", r: "0.8", fill: "currentColor" }
            path { d: "M9 14.5c.8 1 2 1.5 3 1.5s2.2-.5 3-1.5" }
        }
    }
}

#[component]
pub fn IconMonitor() -> Element {
    rsx! {
        svg {
            width: "13",
            height: "13",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.9",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            rect { x: "3", y: "4", width: "18", height: "12", rx: "2" }
            line { x1: "8", y1: "20", x2: "16", y2: "20" }
            line { x1: "12", y1: "16", x2: "12", y2: "20" }
        }
    }
}

#[component]
pub fn IconCheck() -> Element {
    rsx! {
        svg {
            width: "13",
            height: "13",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2.3",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            polyline { points: "20 6 9 17 4 12" }
        }
    }
}

#[component]
pub fn IconAlert() -> Element {
    rsx! {
        svg {
            width: "13",
            height: "13",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            circle { cx: "12", cy: "12", r: "9" }
            line { x1: "12", y1: "8", x2: "12", y2: "12.5" }
            line { x1: "12", y1: "16", x2: "12.01", y2: "16" }
        }
    }
}

#[component]
pub fn IconVolumeOff() -> Element {
    rsx! {
        svg {
            width: "12",
            height: "12",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            polygon { points: "11 5 6 9 2 9 2 15 6 15 11 19 11 5" }
            line { x1: "22", y1: "9", x2: "16", y2: "15" }
            line { x1: "16", y1: "9", x2: "22", y2: "15" }
        }
    }
}
