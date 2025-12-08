# Introduction

### Loading

FrostBar will look for a configuration file at
`$XDG_CONFIG_HOME/frostbar/config.kdl`. If no configuration file exists, a new
one will be created with the contents of the [default configuration
file](https://github.com/Ind-E/FrostBar/blob/main/assets/default-config.kdl).

The config file is live-reloaded. Whenever the file is saved, changes will
automatically be applied. If the config file fails to parse, a notification
will be sent.

### Syntax

The config is written in [KDL](https://kdl.dev).

(Neo)Vim and Helix come with KDL syntax highlighting by default.
VSCode users can install the [KDL
Extension](https://marketplace.visualstudio.com/items?itemName=kdl-org.kdl).

#### Comments

Lines starting with `//` are comments; they are ignored.

Sections starting with `/-` are commented out; Everything in that section is
ignored.

```kdl
/-time {
    // Everything inside here is ignored.
    format "%H\n%M"
}
```

#### Colors

Colors can be specified as hex literals in the form of `#rrggbb`, `#rrggbbaa`,
`#rgb`, or `#rgba` and are case-insensitive.

Example:

```kdl
background "#73F5AB"
```

#### Colors File

In addition to specifying hex codes directly in the main config file, there is
also the option to source colors from a separate file, located at `colors.kdl`
in the same directory as the main `config.kdl` file. This could be useful to
change colors dynamically based on your wallpaper, for example.

The format of this file is a list of color names and hex codes.

Example:

```kdl
color1 "f00"
color2 "858585"
```

Then, in the min `config.kdl` file, you can use one of the defined color names
prefixed with a `$` instead of a hex code.

Example:

```kdl
background: "$color1"
```

If a color name cannot be found in the `colors.kdl` file, it will default to red.

The ability to specift a config file with a command line argument is planned.
