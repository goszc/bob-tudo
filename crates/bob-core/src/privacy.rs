#[derive(Clone, Copy, Debug)]
pub enum RegDesired {
    Dword(u32),
    Sz(&'static str),
}

#[derive(Clone, Copy, Debug)]
pub struct RegEntry {
    pub hive: &'static str,
    pub path: &'static str,
    pub name: &'static str,
    pub when_on: RegDesired,
}

#[derive(Clone, Copy, Debug)]
pub struct PrivacyToggle {
    pub id: &'static str,
    pub title: &'static str,
    pub detail: &'static str,
    pub needs_admin: bool,
    pub entries: &'static [RegEntry],
}

const ADS: &[RegEntry] = &[RegEntry {
    hive: "hkcu",
    path: r"Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo",
    name: "Enabled",
    when_on: RegDesired::Dword(0),
}];

const TAILORED: &[RegEntry] = &[RegEntry {
    hive: "hkcu",
    path: r"Software\Microsoft\Windows\CurrentVersion\Privacy",
    name: "TailoredExperiencesWithDiagnosticDataEnabled",
    when_on: RegDesired::Dword(0),
}];

const SUGGESTIONS: &[RegEntry] = &[
    entry("hkcu", r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager", "SubscribedContent-338388Enabled"),
    entry("hkcu", r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager", "SubscribedContent-338389Enabled"),
    entry("hkcu", r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager", "SubscribedContent-353694Enabled"),
    entry("hkcu", r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager", "SubscribedContent-353696Enabled"),
    entry("hkcu", r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager", "SilentInstalledAppsEnabled"),
    entry("hkcu", r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager", "SystemPaneSuggestionsEnabled"),
    entry("hkcu", r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager", "SoftLandingEnabled"),
];

const BING: &[RegEntry] = &[
    entry("hkcu", r"Software\Microsoft\Windows\CurrentVersion\Search", "BingSearchEnabled"),
    entry("hkcu", r"Software\Microsoft\Windows\CurrentVersion\Search", "CortanaConsent"),
    entry("hkcu", r"Software\Microsoft\Windows\CurrentVersion\Search", "AllowSearchToUseLocation"),
];

const COPILOT: &[RegEntry] = &[
    entry("hkcu", r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced", "ShowCopilotButton"),
    RegEntry {
        hive: "hkcu",
        path: r"Software\Policies\Microsoft\Windows\WindowsCopilot",
        name: "TurnOffWindowsCopilot",
        when_on: RegDesired::Dword(1),
    },
];

const ACTIVITY: &[RegEntry] = &[
    entry("hkcu", r"Software\Microsoft\Windows\CurrentVersion\ActivityHistory\Settings", "EnableActivityFeed"),
    entry("hkcu", r"Software\Microsoft\Windows\CurrentVersion\ActivityHistory\Settings", "PublishUserActivities"),
];

const LOCK: &[RegEntry] = &[
    entry("hkcu", r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager", "RotatingLockScreenOverlayEnabled"),
    entry("hkcu", r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager", "SubscribedContent-338387Enabled"),
];

const FEEDBACK: &[RegEntry] = &[RegEntry {
    hive: "hkcu",
    path: r"Software\Microsoft\Siuf\Rules",
    name: "NumberOfSIUFInPeriod",
    when_on: RegDesired::Dword(0),
}];

const LOCATION: &[RegEntry] = &[RegEntry {
    hive: "hkcu",
    path: r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\location",
    name: "Value",
    when_on: RegDesired::Sz("Deny"),
}];

const INKING: &[RegEntry] = &[
    RegEntry {
        hive: "hkcu",
        path: r"Software\Microsoft\InputPersonalization",
        name: "RestrictImplicitInkCollection",
        when_on: RegDesired::Dword(1),
    },
    RegEntry {
        hive: "hkcu",
        path: r"Software\Microsoft\InputPersonalization",
        name: "RestrictImplicitTextCollection",
        when_on: RegDesired::Dword(1),
    },
    RegEntry {
        hive: "hkcu",
        path: r"Software\Microsoft\InputPersonalization\TrainedDataStore",
        name: "HarvestContacts",
        when_on: RegDesired::Dword(0),
    },
];

const TELEMETRY: &[RegEntry] = &[RegEntry {
    hive: "hklm",
    path: r"SOFTWARE\Policies\Microsoft\Windows\DataCollection",
    name: "AllowTelemetry",
    when_on: RegDesired::Dword(0),
}];

const START: &[RegEntry] = &[entry(
    "hkcu",
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
    "Start_IrisRecommendations",
)];

const fn entry(hive: &'static str, path: &'static str, name: &'static str) -> RegEntry {
    RegEntry {
        hive,
        path,
        name,
        when_on: RegDesired::Dword(0),
    }
}

pub fn privacy_toggles() -> &'static [PrivacyToggle] {
    &TOGGLES
}

const TOGGLES: &[PrivacyToggle] = &[
    PrivacyToggle {
        id: "ads-id",
        title: "bloquear id de publicidade",
        detail: "o windows deixa de usar um identificador para anúncios dos aplicativos.",
        needs_admin: false,
        entries: ADS,
    },
    PrivacyToggle {
        id: "tailored",
        title: "bloquear experiências sob medida",
        detail: "para de usar dados de diagnóstico para dicas personalizadas.",
        needs_admin: false,
        entries: TAILORED,
    },
    PrivacyToggle {
        id: "suggestions",
        title: "bloquear sugestões e anúncios",
        detail: "tira dicas, aplicativos sugeridos e anúncios do menu iniciar e das configurações.",
        needs_admin: false,
        entries: SUGGESTIONS,
    },
    PrivacyToggle {
        id: "bing",
        title: "desligar busca na web e cortana",
        detail: "a pesquisa do iniciar deixa de consultar a web e a cortana.",
        needs_admin: false,
        entries: BING,
    },
    PrivacyToggle {
        id: "copilot",
        title: "esconder o copilot",
        detail: "esconde o botão do copilot. não desinstala o aplicativo.",
        needs_admin: false,
        entries: COPILOT,
    },
    PrivacyToggle {
        id: "activity",
        title: "desligar histórico de atividades",
        detail: "para a linha do tempo e a coleta do histórico de atividades.",
        needs_admin: false,
        entries: ACTIVITY,
    },
    PrivacyToggle {
        id: "lockscreen",
        title: "desligar dicas na tela de bloqueio",
        detail: "tira as dicas e os cartões da tela de bloqueio.",
        needs_admin: false,
        entries: LOCK,
    },
    PrivacyToggle {
        id: "feedback",
        title: "desligar pedidos de feedback",
        detail: "o windows para de pedir avaliações e comentários.",
        needs_admin: false,
        entries: FEEDBACK,
    },
    PrivacyToggle {
        id: "location",
        title: "negar localização dos aplicativos",
        detail: "os aplicativos deixam de ter permissão geral de localização.",
        needs_admin: false,
        entries: LOCATION,
    },
    PrivacyToggle {
        id: "inking",
        title: "limitar coleta de tinta e digitação",
        detail: "o windows para de aprender com o que você escreve.",
        needs_admin: false,
        entries: INKING,
    },
    PrivacyToggle {
        id: "telemetry",
        title: "reduzir telemetria",
        detail: "pede administrador. deixa o diagnóstico enviado à microsoft no mínimo.",
        needs_admin: true,
        entries: TELEMETRY,
    },
    PrivacyToggle {
        id: "start-recs",
        title: "desligar recomendações do iniciar",
        detail: "remove as recomendações de arquivos e sites no menu iniciar.",
        needs_admin: false,
        entries: START,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<_> = privacy_toggles().iter().map(|item| item.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), privacy_toggles().len());
        assert!(privacy_toggles().iter().any(|item| item.needs_admin));
    }
}
