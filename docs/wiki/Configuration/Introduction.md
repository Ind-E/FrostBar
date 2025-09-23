# Introduction

### Loading

FrostBar will look for a configuration file at
`$XDG_CONFIG_HOME/frostbar/config.kdl`. If no configuration file exists, a new
one will be created with the contents of the [default configuration
file](https://github.com/Ind-E/FrostBar/blob/main/assets/default-config.kdl).

The config file is live-reloaded. Whenever the file is saved, changes will
automatically be applied. If the config file fails to parse, A notification
will be sent.

### Syntax

The config is written in [KDL](https://kdl.dev).

#### Colors

Colors can be specified as either:

- Hex literals in the form of `#rrggbb`, `#rrggbbaa`, `#rgb`, or `#rgba`.

- Component values as space-separated numbers (`R G B` or `R G B A`, with alpha
defaulting to 1.0).

Example:

```kdl
battery {
    // As a hex literal
    charging-color "#73F5AB"

    // As RGB with alpha
    charging-color 115 244 170 1.0

    // As RGB only (alpha defaults to 1.0)
    charging-color 115 244 170
}
```

#### Comments

Lines starting with `//` are comments; they are ignored.


Also, you can put `/-` in front of a section to comment out the entire section:

```kdl
/-time {
    // Everything inside here is ignored.
    format "%H\n%M"
}
```

## Planned Features
- change bar position (left, top, right, bottom)
- specify config file with cli argument
- change colors/styles of each module
