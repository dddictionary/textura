{
  description = "textura - convert images, video, and live camera into ASCII art";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      systems = [ "aarch64-darwin" "x86_64-darwin" "x86_64-linux" "aarch64-linux" ];
      forEachSystem = f:
        nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      devShells = forEachSystem (pkgs: {
        default = pkgs.mkShell {
          # ffmpeg-sys-next binds against the libav* headers, so we pin the
          # major version to match the `ffmpeg-next` crate (8.x). nixpkgs'
          # default `ffmpeg` is 9.x, which the crate does not target yet.
          nativeBuildInputs = with pkgs; [
            rustc
            cargo
            rustfmt
            clippy
            rust-analyzer
            pkg-config
          ];

          buildInputs = with pkgs; [
            ffmpeg_8
          ];

          # bindgen needs libclang at runtime to parse the ffmpeg headers.
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";

          shellHook = ''
            echo "textura dev shell - rustc $(rustc --version | cut -d' ' -f2), ffmpeg $(ffmpeg -version | head -1 | cut -d' ' -f3)"

            # `nix develop` drops you into its own bash, which loses your
            # prompt and config. Re-exec into your real shell instead.
            #
            # $SHELL is useless here (nix has already overwritten it with its
            # own bash), hence the hardcoded default. Override with
            # TEXTURA_SHELL if yours lives elsewhere.
            #
            # Guarded three ways: only when interactive, so `nix develop -c
            # <cmd>` still runs <cmd>; only once, so re-entering can't loop;
            # and only if the binary actually exists, so a missing shell
            # leaves you in bash rather than a broken shell.
            _textura_shell="''${TEXTURA_SHELL:-/bin/zsh}"
            if [[ $- == *i* && -z "''${IN_TEXTURA_SHELL:-}" && -x "$_textura_shell" ]]; then
              export IN_TEXTURA_SHELL=1
              exec "$_textura_shell"
            fi
          '';
        };
      });
    };
}
