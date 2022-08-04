{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.rust-overlay.url = "github:oxalica/rust-overlay";

  outputs = { self, nixpkgs, rust-overlay }:
    let
      eachSystem = nixpkgs.lib.genAttrs [ "x86_64-linux" ];
    in {
      devShells = eachSystem (system: {
        default =
          let
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ rust-overlay.overlays.default ];
            };

            env = { mkShell, rust-bin }:
              mkShell {
                nativeBuildInputs = [
                  (rust-bin.stable.latest.default.override {
                    extensions = [ "rust-src" ];
                    targets = [ "x86_64-unknown-linux-gnu" ];
                  })
                ];
              };
          in pkgs.callPackage env {};
      });
    };
}
