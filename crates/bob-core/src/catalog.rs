use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OssApp {
    pub name: String,
    pub description: String,
    pub category: String,
    pub repo: String,
    pub winget_id: String,
    pub alternative_to: String,
    pub stars: u64,
}

pub fn oss_catalog() -> Vec<OssApp> {
    APPS.iter()
        .map(|app| OssApp {
            name: app.0.into(),
            description: app.1.into(),
            category: app.2.into(),
            repo: app.3.into(),
            winget_id: app.4.into(),
            alternative_to: app.5.into(),
            stars: 0,
        })
        .collect()
}

/// nome, descrição, categoria, repo, id do winget, alternative a
const APPS: &[(&str, &str, &str, &str, &str, &str)] = &[
    ("firefox", "navegador da mozilla.", "navegador", "mozilla-firefox/firefox", "Mozilla.Firefox", "alternative ao chrome"),
    ("librewolf", "firefox com menos rastreio.", "navegador", "LibreWolf/librewolf", "LibreWolf.LibreWolf", "alternative ao chrome"),
    ("brave", "navegador com bloqueio de anúncios.", "navegador", "brave/brave-browser", "Brave.Brave", "alternative ao chrome"),
    ("tor browser", "navegador para navegar com mais privacidade.", "navegador", "torproject/tor-browser", "TorProject.TorBrowser", "alternative ao chrome"),
    ("vlc", "abre quase qualquer vídeo e áudio.", "vídeo", "videolan/vlc", "VideoLAN.VLC", "alternative ao windows media player"),
    ("mpv", "player leve, pelo teclado.", "vídeo", "mpv-player/mpv", "mpv.net", "alternative ao windows media player"),
    ("obs studio", "grava a tela e transmite ao vivo.", "vídeo", "obsproject/obs-studio", "OBSProject.OBSStudio", ""),
    ("kdenlive", "editor de vídeo.", "vídeo", "KDE/kdenlive", "KDE.Kdenlive", "alternative ao premiere"),
    ("shotcut", "editor de vídeo simples.", "vídeo", "mltframework/shotcut", "Meltytech.Shotcut", "alternative ao premiere"),
    ("handbrake", "converte vídeos.", "vídeo", "HandBrake/HandBrake", "HandBrake.HandBrake", "alternative ao conversor pago"),
    ("audacity", "grava e edita áudio.", "áudio", "audacity/audacity", "Audacity.Audacity", "alternative ao audition"),
    ("musescore", "escreve partitura.", "áudio", "musescore/MuseScore", "Musescore.Musescore", ""),
    ("gimp", "editor de imagem.", "imagem", "GNOME/gimp", "GIMP.GIMP", "alternative ao photoshop"),
    ("krita", "desenho e pintura.", "imagem", "KDE/krita", "KDE.Krita", "alternative ao photoshop"),
    ("inkscape", "desenho vetorial.", "imagem", "inkscape/inkscape", "Inkscape.Inkscape", "alternative ao illustrator"),
    ("darktable", "revela fotos raw.", "imagem", "darktable-org/darktable", "darktable.darktable", "alternative ao lightroom"),
    ("blender", "3d, do modelo à animação.", "imagem", "blender/blender", "BlenderFoundation.Blender", ""),
    ("sharex", "captura de tela e gravação.", "imagem", "ShareX/ShareX", "ShareX.ShareX", "alternative ao lightshot"),
    ("flameshot", "captura de tela com anotações.", "imagem", "flameshot-org/flameshot", "Flameshot.Flameshot", "alternative ao lightshot"),
    ("libreoffice", "texto, planilha e apresentação.", "escritório", "LibreOffice/core", "TheDocumentFoundation.LibreOffice", "alternative ao microsoft office"),
    ("onlyoffice", "edita documentos do office.", "escritório", "ONLYOFFICE/DesktopEditors", "ONLYOFFICE.DesktopEditors", "alternative ao microsoft office"),
    ("sumatra pdf", "lê pdf e ebook, bem leve.", "escritório", "sumatrapdfreader/sumatrapdf", "SumatraPDF.SumatraPDF", "alternative ao adobe reader"),
    ("joplin", "notas e listas.", "escritório", "laurent22/joplin", "Joplin.Joplin", "alternative ao evernote"),
    ("7-zip", "compacta e abre zip, 7z e rar.", "arquivos", "ip7z/7zip", "7zip.7zip", "alternative ao winrar"),
    ("peazip", "compactador com vários formatos.", "arquivos", "peazip/PeaZip", "Giorgiotani.Peazip", "alternative ao winrar"),
    ("qbittorrent", "cliente de torrent.", "arquivos", "qbittorrent/qBittorrent", "qBittorrent.qBittorrent", "alternative ao utorrent"),
    ("transmission", "cliente de torrent simples.", "arquivos", "transmission/transmission", "Transmission.Transmission", "alternative ao utorrent"),
    ("localsend", "envia arquivos na rede local.", "arquivos", "localsend/localsend", "LocalSend.LocalSend", "alternative ao airdrop"),
    ("syncthing", "sincroniza pastas sem nuvem.", "arquivos", "syncthing/syncthing", "Syncthing.Syncthing", "alternative ao dropbox"),
    ("nextcloud", "cliente da sua nuvem.", "arquivos", "nextcloud/desktop", "Nextcloud.NextcloudDesktop", "alternative ao onedrive"),
    ("keepassxc", "guarda senhas num cofre local.", "segurança", "keepassxreboot/keepassxc", "KeePassXCTeam.KeePassXC", "alternative ao 1password"),
    ("bitwarden", "cofre de senhas.", "segurança", "bitwarden/clients", "Bitwarden.Bitwarden", "alternative ao 1password"),
    ("bleachbit", "limpa arquivos temporários.", "segurança", "bleachbit/bleachbit", "BleachBit.BleachBit", ""),
    ("wireshark", "inspeciona o tráfego de rede.", "segurança", "wireshark/wireshark", "WiresharkFoundation.Wireshark", ""),
    ("thunderbird", "e-mail e calendário.", "comunicação", "mozilla/thunderbird", "Mozilla.Thunderbird", "alternative ao outlook"),
    ("signal", "mensagens criptografadas.", "comunicação", "signalapp/Signal-Desktop", "OpenWhisperSystems.Signal", "alternative ao whatsapp"),
    ("vscodium", "editor de código sem a telemetria da microsoft.", "desenvolvimento", "VSCodium/vscodium", "VSCodium.VSCodium", "alternative ao visual studio code"),
    ("neovim", "editor de texto no terminal.", "desenvolvimento", "neovim/neovim", "Neovim.Neovim", ""),
    ("git", "controle de versão.", "desenvolvimento", "git/git", "Git.Git", ""),
    ("alacritty", "terminal rápido.", "desenvolvimento", "alacritty/alacritty", "Alacritty.Alacritty", ""),
    ("wezterm", "terminal com abas e gpu.", "desenvolvimento", "wezterm/wezterm", "wez.wezterm", ""),
    ("dbeaver", "cliente de banco de dados.", "desenvolvimento", "dbeaver/dbeaver", "DBeaver.DBeaver", ""),
    ("windows terminal", "terminal moderno do windows.", "desenvolvimento", "microsoft/terminal", "Microsoft.WindowsTerminal", ""),
    ("powershell", "shell e automação.", "desenvolvimento", "PowerShell/PowerShell", "Microsoft.PowerShell", ""),
    ("powertoys", "ferramentas extras da microsoft.", "utilitário", "microsoft/PowerToys", "Microsoft.PowerToys", ""),
    ("flow launcher", "busca rápida no teclado.", "utilitário", "Flow-Launcher/Flow.Launcher", "Flow-Launcher.Flow-Launcher", "alternative ao alfred"),
    ("eartrumpet", "volume por aplicativo.", "utilitário", "File-New-Project/EarTrumpet", "File-New-Project.EarTrumpet", ""),
    ("translucenttb", "deixa a barra de tarefas translúcida.", "utilitário", "TranslucentTB/TranslucentTB", "CharlesMilette.TranslucentTB", ""),
    ("notepad++", "editor de texto.", "utilitário", "notepad-plus-plus/notepad-plus-plus", "Notepad++.Notepad++", "alternative ao bloco de notas"),
    ("putty", "cliente de ssh.", "utilitário", "", "PuTTY.PuTTY", ""),
    ("winscp", "copia arquivos por sftp.", "utilitário", "winscp/winscp", "WinSCP.WinSCP", ""),
    ("rufus", "cria pendrive de instalação.", "utilitário", "pbatard/rufus", "Rufus.Rufus", ""),
    ("ventoy", "vários isos num pendrive.", "utilitário", "ventoy/Ventoy", "Ventoy.Ventoy", ""),
    ("balena etcher", "grava imagem no pendrive.", "utilitário", "balena-io/etcher", "Balena.Etcher", ""),
    ("playnite", "biblioteca dos seus jogos.", "utilitário", "JosefNemec/Playnite", "Playnite.Playnite", ""),
    ("winmerge", "compara arquivos e pastas.", "utilitário", "WinMerge/winmerge", "WinMerge.WinMerge", ""),
    ("kate", "editor de texto do kde.", "utilitário", "KDE/kate", "KDE.Kate", ""),
];
