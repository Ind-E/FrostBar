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

Colors can be specified as either:

- Hex literals in the form of `#rrggbb`, `#rrggbbaa`, `#rgb`, or `#rgba`.

- Component values as space-separated numbers (`R G B` or `R G B A`, with alpha
defaulting to 1.0).

Example:

```kdl
// As a hex literal
background "#73F5AB"

// As RGB with alpha
background 115 244 170 1.0

// As RGB only (alpha defaults to 1.0)
background 115 244 170
```

## Planned Features
- change bar position (left, top, right, bottom)
- specify config file with cli argument
- change colors/styles of each module
