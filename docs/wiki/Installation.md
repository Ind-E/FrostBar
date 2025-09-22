# Installation


### Nix

First, add the repository to your flake inputs:
```nix
inputs = {
  frostbar.url = "github:Ind-E/FrostBar";

  # ...
};
```

Then, add it to `environment.systemPackages`:

```nix
{
  pkgs,
  inputs,
  system,
  ...
}:
{
  environment.systemPackages = with pkgs; [

    inputs.frostbar.packages.${system}.default
    # ...
  ]
  # ...
}
```

### Cargo

First, clone the repository locally:
```sh
git clone https://github.com/Ind-E/FrostBar
```

Then, navigate into the `FrostBar` directory and install using cargo:

```sh
cargo install --path .
```



