# Modules

FrostBar comes with a variety of modules that you can make use of. Each
module may be added any number of times to the bar with different
configurations for each.

Here is a list of all currently avaialable modules (as of 1/24/26):


```kdl
battery
audio-visualizer
label
mpris
niri
time
```

## Common Configuration Options

These are the set of configuration options that are common across (almost) all modules.

### Mouse Binds

Mouse binds allow running commands when you interact with a widget with your
mouse by clicking or scrolling.
Options include:

```kdl
mouse-left
mouse-right
mouse-middle
scroll-up
scroll-down
scroll-left
scroll-right
```

After the mouse bind, you can specify the command to be run in 2 ways.

The first way will NOT run the command in a shell and requires separating
arguments in different string literals.
```kdl
scroll-up "wpctl" "set-volume" "@DEFAULT_SINK@" "3%+"
```

The second way will run in a shell, allowing access to pipes and subshells.
```kdl
scroll-up sh=true "wpctl set-volume @DEFAULT_SINK@ 3%+ && notify-send volume+"
```

### Container Style

This allows customizing the style of the container surrounding a widget.
Options include:


```kdl
style {
    text-color "#fff"
    background "#000"
    padding 5
    border {
        color "#fff"
        width 0.5
        radius 10.0
    }
}
```

Colors may be specified as described by the [Colors
Section](Introduction.md#colors).

#### text-color
Color of text inside the widget.

#### background
Background color of the widget.

#### padding
Inner margin for items inside the container

#### border
Color, radius, and width of the border around the widget.

The border radius may also be specified per-corner like so:
```kdl
border {
    radius {
        top-left 10.0
        top-right 0.0
        bottom-left 0.0
        bottom-right 10.0
    }
}

```


## Module Specific Configuration Options

### Battery
```kdl
battery {
    icon-size 22
    charging-color "#73F5AB"
}
```

#### icon-size
Size of the battery icon.

#### charging-color
Color of the battery while plugged in. Use `text-color` in the
`style` section to set the color while not plugged in.

### Audio Visualizer
```kdl
audio-visualizer {
    spacing 0.1
    length 130
    dynamic-color true
    color "#fff"
}
```

#### spacing
Spacing between bars in the audio visualizer. Can be specified
either as a percentage, or in logical units.
```kdl
// 2 logical units
spacing 2

// 10% of widget length
spacing "20%"
```

#### length
Length of the audio visualizer widget.

#### dynamic-color
Whether or not to source colors from the currently playing song's album art.
Defaults to true if not included.

#### color
if `dynamic-color` is enabled, sets the color of the bars when no album art
is available. Otherwise, sets the color of the bars at all times.

### Label
```kdl
label {
    text "text"
    size 22
    tooltip "tooltip"
}
```

#### text
Text to be displayed by the label.

#### size
Size of the label text.

#### tooltip
Text that appears in a tooltip when hovering over the label.

### Mpris

For every detected mpris compatible player, its album art will be displayed.
If no album art is available, the placeholder will be shown instead.
`placeholder-style` has the same options as the [Container
Style](#container-style) section. The mpris module does not support the
generic `style` options.

For the mpris module, instead of running commands with mouse binds, mpris
specific actions can be specified.

Available actions:
```kdl
"play"
"pause"
// if currently playing, pause. If currently paused, play
"play-pause"
"next"
"previous"
"stop"
// in milliseconds. Can be negative
"seek" 100
// decrease volume by 5%
"volume" -0.05
"set-volume" 0
```

Here is an example config:
```kdl
mpris {
    mouse-left "play-pause"
    scroll-right "seek" 5000
    scroll-left "seek" -5000
    mouse-right "next"
    mouse-middle "stop"

    placeholder "󰝚"
    placeholder-style {
        border {
            width 1.0
            color "#fff"
            radius 0.0
        }
    }
}
```

### Niri

Displays information about windows and workspaces from the niri compositor.

```kdl
niri {
    spacing 10
    workspace-active-style {
        text-color "#fff"
        border {
            color "#fff"
            width 3.0
            radius 0.0
        }
    }
    workspace-hovered-style {
        background "#aaa4"
    }

    workspace-offset -1
}
```

#### spacing
Spacing between workspaces.


#### styles
There are 4 different styles for different parts of the niri widget:
`window-focused-style`, `window-style`, `workspace-active-style`,
`workspace-hovered-style`, and `workspace-style`. All niri styles have the same
options as the [Container Style](#container-style) section.

Niri styles are merged according to their priority. FrostBar will use
available options from higher priority styles first and fallback to lower
ones if they are unset, eventually falling back to the default style.

Niri styles have the following priority:

- `workspace-hovered-style`
- `workspace-active-style`
- `workspace-style`
- `window-focused-style`
- `window-style`

#### workspace-offset
Offset to apply to the index of each workspace. I use this with niri's
`empty-workspace-above-first` option to start labeling workspaces at 0 instead of 1.


### Time
```kdl
time {
    format "%H\n%M"
    tooltip-format "%a %b %-d\n%-m/%-d/%y"
}
```

#### format
Format string for displaying the time. See the [chrono
documentation](https://docs.rs/chrono/latest/chrono/format/strftime/index.html)
for information on format specifiers.

#### tooltip-format
Format string for displaying the tooltip. See the [chrono
documentation](https://docs.rs/chrono/latest/chrono/format/strftime/index.html)
for information on format specifiers.
