{
  description = "A Leptos dashboard for Docker services";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs =
    {
      self,
      nixpkgs,
      ...
    }:
    let
      systems = [
        "aarch64-linux"
        "x86_64-linux"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
      perSystem =
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          tools = [
            pkgs.cargo-leptos
            pkgs.rustc.llvmPackages.lld
            # Must match the wasm-bindgen version in Cargo.lock.
            pkgs.wasm-bindgen-cli_0_2_128
            pkgs.binaryen
          ];
          package = pkgs.rustPlatform.buildRustPackage {
            pname = "webdash-rs";
            version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;
            src = pkgs.lib.fileset.toSource {
              root = ./.;
              fileset = pkgs.lib.fileset.unions [
                ./Cargo.toml
                ./Cargo.lock
                ./src
                ./assets
                ./style.css
              ];
            };
            cargoLock.lockFile = ./Cargo.lock;
            nativeBuildInputs = tools ++ [ pkgs.makeWrapper ];

            buildPhase = ''
              runHook preBuild
              cargo leptos build --release
              runHook postBuild
            '';

            installPhase = ''
              runHook preInstall
              mkdir -p $out/bin $out/share/webdash-rs
              cp target/release/webdash-rs $out/bin/
              cp -r target/site $out/share/webdash-rs/site
              wrapProgram $out/bin/webdash-rs \
                --set-default LEPTOS_SITE_ROOT $out/share/webdash-rs/site \
                --set-default LEPTOS_SITE_PKG_DIR pkg \
                --set-default LEPTOS_OUTPUT_NAME webdash-rs \
                --set-default SSL_CERT_FILE ${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt
              runHook postInstall
            '';

            meta.mainProgram = "webdash-rs";
          };
        in
        {
          inherit package;
          shell = pkgs.mkShell {
            packages = [
              pkgs.cargo
              pkgs.rustc
              pkgs.clippy
              pkgs.rustfmt
            ]
            ++ tools;
          };
        };
    in
    {
      packages = forAllSystems (system: {
        default = (perSystem system).package;
      });
      checks = self.packages;
      devShells = forAllSystems (system: {
        default = (perSystem system).shell;
      });
    };
}
