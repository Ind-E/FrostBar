# Introduction

### Loading

FrostBar will look for a configuration file at
`$XDG_CONFIG_HOME/frostbar/config.kdl`. If no configuration file exists, a new
one will be created with the contents of the [default configuration
file](https://github.com/Ind-E/FrostBar/blob/main/assets/default-config.kdl)

The config file is live-reloaded. Whenever the file is saved, changes will
automatically be applied. If the config file fails to parse, you will get a
notification about it.

### Syntax

The config is written in [KDL](https://kdl.dev).

### Colors

Colors can be of the form `#rrggbb`, `#rrggbbaa`, `#rgb`, or `#rgba`

#### Comments

Lines starting with `//` are comments; they are ignored.




## Planned Features
- hot reloading of config file
- validation of config file with error notification
- change bar position (left, top, right, bottom)
- specify config file with cli argument
- change colors/styles of each module
- ability to position modules at start, middle, or end of bar, and to use any combination of modules
