# Packaging

## Homebrew

Build from the formula:

```sh
brew install --build-from-source packaging/homebrew/suture.rb
```

Or install from a tap (once published):

```sh
brew tap WyattAu/suture
brew install suture
```

## Arch Linux (AUR)

Clone the AUR package, build, and install:

```sh
git clone https://aur.archlinux.org/suture.git
cd suture
makepkg -si
```

Or build from this repository:

```sh
cd packaging/aur
makepkg -si
```
