export const REPO = 'https://github.com/usecornercase/cornercase-gpui';
export const RELEASES = `${REPO}/releases/latest`;
export const DOWNLOAD = `${REPO}/releases/latest/download`;
export const CORNERCASE = 'https://usecornercase.dev';
export const DOCS = `${CORNERCASE}/docs/installation/`;
export const DISCORD = 'https://discord.gg/aYqCa4xC4R';

export const FILES = {
  mac: 'cornercase-gpui-universal-apple-darwin.dmg',
  windows: 'cornercase-gpui-x86_64-pc-windows-msvc.zip',
  linux: 'cornercase-gpui-x86_64-unknown-linux-gnu.tar.gz',
};

export const INSTALLS: [string, string][] = [
  ['Install script', 'curl -fsSL https://usecornercase.dev/install.sh | sh'],
  ['Homebrew', 'brew install usecornercase/tap/cornercase'],
  ['Windows', 'powershell -ExecutionPolicy Bypass -c "irm https://usecornercase.dev/install.ps1 | iex"'],
];
