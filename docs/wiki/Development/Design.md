# Design Philosophies

### Fast
One of the only things that makes FrostBar stand out from its
competitors is it's speed. It's written entirely in rust, and doesn't use a
bloated UI toolkit. It's important that it stays fast and that new features are
performant.

### Users Don't Pay for Unused Features
Features that aren't enabled in the config file should have little to no impact on the performance when those features are disabled.

### No Required System Dependencies 
As much as possible, I'd like to eliminate
the number of dependencies that users have to install to get FrostBar working.
This is to make the installation process easier, and also reduce any problems
between version mismatches. It's obviously impossible to have 0 dependencies,
niri (or another wayland compositor) being the biggest one, but for features
such as audio visualization or cpu monitoring, there should be a version that
works without needing other dependencies installed. I'm not opposed to having
optional dependencies, but I probably won't implement them myself.

### Linux Wayland First
 FrostBar has been designed to work only on linux
machines with wayland compositors, and even if there are certain parts that
would work on other operating systems, I won't be focusing on that. PRs are
still welcome, but they aren't a high priority.
