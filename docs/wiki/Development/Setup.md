# Setup

### Step 1: Clone the repository

```sh
git clone https://github.com/Ind-E/FrostBar
```

### Step 2: Install Dependencies

#### Nix

If using nix, run `nix develop` to enter a development shell with all needed
dependencies. You could also use `direnv`.

#### Non-Nix

TODO

### Step 3: Build and Run Locally

To run a development version locally, use `cargo run`

To run an optimized version, use `cargo run --release`

To enable profiling with Tracy, use `cargo run --features tracy`
