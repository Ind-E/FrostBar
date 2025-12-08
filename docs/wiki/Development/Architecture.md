# Architecture

FrostBar is written in rust and made with
[iced](https://github.com/iced-rs/iced). 

The documentation is built with [MkDocs](https://www.mkdocs.org) and uses the
[Material for MkDocs](https://squidfunk.github.io/mkdocs-material/) theme[^1].

Most of the components of the bar (e.g. battery, time, workspaces) are in
self-containing modules.  Each module has 2 components - a service and a view.
A service runs in the background, and there will be at most 1 of a service
running at a time. A view takes data from its corresponding service as input
and displays it as a gui element. In this way, it is effortless to display
multiple widgets built from the same data. For example, you could have 2 clocks
that show the time and date respectively, both being updated from the same
service, as each view can have a different configuration.

Not all modules are entirely self-contained, however. Right now, the audio
visualizer module depends on the mpris module in order to pick colors from the
album art of the currently playing song. In order to make cases like this
better in the future, I'd like to develop a subscribtion architechture, where
services can start and stop listening to events from other services
dynamically. (This is complicated by the fact that services could start or stop
when the hot-reloaded config file is changed).


[^1]: 
    As of 11/11/25, Material for MkDocs is in maintenance mode. I will look
    into migrating documentation to a new project from the same developers called
    Zensical once it becomes more stable.
