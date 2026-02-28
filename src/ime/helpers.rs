use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Globalization::*;
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ,
};
use windows::Win32::UI::Input::Ime::*;
use windows::Win32::UI::Input::KeyboardAndMouse::HKL;
use windows::Win32::UI::Shell::SHLoadIndirectString;
use windows::Win32::UI::WindowsAndMessaging::*;

#[allow(dead_code)]
pub(super) fn get_imm_description(hkl: HKL) -> Option<String> {
    let mut buffer = [0u16; 128];
    let len = unsafe { ImmGetDescriptionW(hkl.clone(), Some(&mut buffer)) };
    if len > 0 {
        println!(
            "[IME] ImmGetDescriptionW: {:?}",
            String::from_utf16_lossy(&buffer[..len as usize])
        );
        Some(String::from_utf16_lossy(&buffer[..len as usize]))
    } else {
        None
    }
}

///
/// # Arguments
/// * `lang_id` - LCID https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-lcid/63d3d639-7fd2-4afb-abbe-0d5b5551eef8
/// * `lctype` - The type of locale information to retrieve (e.g., LOCALE_SLANGUAGE)
pub(super) fn get_locate_info(lang_id: u16, lctype: u32) -> Option<String> {
    let mut buffer = [0u16; 128];
    let len = unsafe { GetLocaleInfoW(lang_id as u32, lctype, Some(&mut buffer)) };
    if len > 0 {
        let result = String::from_utf16_lossy(&buffer[..(len - 1) as usize]);
        // println!("[IME] GetLocaleInfoW: {:?}", result);
        Some(result)
    } else {
        None
    }
}

pub(super) fn get_locate_language(lang_id: u16) -> Option<String> {
    get_locate_info(lang_id, LOCALE_SLANGUAGE)
}

// https://learn.microsoft.com/en-us/windows/win32/intl/language-identifiers
#[allow(dead_code)]
pub(super) struct KeyboardLayout {
    // HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Control\Keyboard Layouts
    pub layout_id: u32,
    pub sub_id: u16,
}

#[allow(dead_code)]
pub(super) fn get_locate_keyboard_install(lang_id: u16) -> Option<KeyboardLayout> {
    if let Some(result) = get_locate_info(lang_id, LOCALE_SKEYBOARDSTOINSTALL) {
        let parts: Vec<&str> = result.split(':').collect();
        if parts.len() != 2 {
            return None;
        }

        let sub_id = u16::from_str_radix(parts[0], 16).ok()?;
        let layout_id = u32::from_str_radix(parts[1], 16).ok()?;

        Some(KeyboardLayout { layout_id, sub_id })
    } else {
        None
    }
}

pub(super) struct LanguageInfo {
    pub main: u16,
    pub sub: u16,
}

pub(super) fn get_lang_info(lang_id: HKL) -> Option<LanguageInfo> {
    let lang_id = lang_id.0 as u32;
    let lang_info = LanguageInfo {
        main: (lang_id & 0xFFFF) as u16,
        sub: (lang_id >> 16) as u16,
    };
    Some(lang_info)
}

#[allow(dead_code)]
pub(super) fn get_registry_layout_id(hkl: HKL) -> String {
    let hkl_val = (hkl.0 as usize & 0xFFFFFFFF) as u32;
    let high = hkl_val >> 16;
    let low = hkl_val & 0xFFFF;

    if high == 0 || high == low {
        format!("0000{:04X}", low)
    } else {
        format!("0000{:04X}", high)
    }
}

#[allow(dead_code)]
pub(super) fn get_keyboard_layout_name_from_registry(hkl: HKL) -> Option<String> {
    unsafe {
        let layout_id = get_registry_layout_id(hkl);
        let sub_key = format!("SYSTEM\\CurrentControlSet\\Control\\Keyboard Layouts\\{}", layout_id);

        let mut hkey = HKEY::default();
        let sub_key_u16: Vec<u16> = sub_key.encode_utf16().chain(Some(0)).collect();

        if RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR::from_raw(sub_key_u16.as_ptr()),
            Some(0),
            KEY_READ,
            &mut hkey,
        )
        .is_err()
        {
            return None;
        }

        let mut buffer = [0u16; 256];
        let mut cb_data = (buffer.len() * 2) as u32;
        let mut result = None;

        // 讀取顯示名稱
        if RegQueryValueExW(
            hkey,
            w!("Layout Display Name"),
            None,
            None,
            Some(buffer.as_mut_ptr() as *mut _),
            Some(&mut cb_data),
        )
        .is_ok()
        {
            let mut out_buffer = [0u16; 256];
            if SHLoadIndirectString(PCWSTR::from_raw(buffer.as_ptr()), &mut out_buffer, None).is_ok() {
                let str = String::from_utf16_lossy(&out_buffer)
                    .trim_matches(char::from(0))
                    .to_string();
                println!("[IME] Found layout display name: {}", &str);
                result = Some(str);
            }
        }

        let _ = RegCloseKey(hkey);
        result
    }
}

pub(super) fn get_open_status(ime_hwnd: HWND) -> bool {
    const IMC_GETOPENSTATUS: u32 = 0x0005;

    let mut open_res = 0usize;

    unsafe {
        let _ = SendMessageTimeoutW(
            ime_hwnd,
            WM_IME_CONTROL,
            WPARAM(IMC_GETOPENSTATUS as usize),
            LPARAM(0),
            SMTO_ABORTIFHUNG | SMTO_NORMAL,
            100,
            Some(&mut open_res),
        );
    }

    open_res != 0
}

#[allow(dead_code)]
pub(super) fn set_open_status(ime_hwnd: HWND, open: bool) {
    const IMC_SETOPENSTATUS: u32 = 0x0006;

    unsafe {
        let result = SendMessageTimeoutW(
            ime_hwnd,
            WM_IME_CONTROL,
            WPARAM(IMC_SETOPENSTATUS as usize),
            LPARAM(open as isize),
            SMTO_ABORTIFHUNG | SMTO_NORMAL,
            100,
            None,
        );

        println!("[IME] SetOpenStatus: {:?}", result);
    }
}

pub(super) fn get_conv_mode(ime_hwnd: HWND) -> IME_CONVERSION_MODE {
    const IMC_GETCONVERSIONMODE: u32 = 0x0001;

    let mut conv_res = 0usize;
    unsafe {
        let _ = SendMessageTimeoutW(
            ime_hwnd,
            WM_IME_CONTROL,
            WPARAM(IMC_GETCONVERSIONMODE as usize),
            LPARAM(0),
            SMTO_ABORTIFHUNG | SMTO_NORMAL,
            100,
            Some(&mut conv_res),
        );
    }
    IME_CONVERSION_MODE(conv_res as u32)
}

#[allow(dead_code)]
pub(super) fn debug_locale_info(lang_id: u32) {
    macro_rules! print_locale {
        ($lctype:expr) => {
            let mut buffer = [0u16; 256];
            let len = unsafe { GetLocaleInfoW(lang_id, $lctype, Some(&mut buffer)) };
            if len > 0 {
                let result = String::from_utf16_lossy(&buffer[..(len - 1) as usize]);
                println!("[IME] {:?}: {:?}", stringify!($lctype), result);
            } else {
                println!("[IME] {:?}: Failed", stringify!($lctype));
            }
        };
    }

    // print_locale!(LOCALE_FONTSIGNATURE);
    // print_locale!(LOCALE_ICALENDARTYPE);
    // print_locale!(LOCALE_ICENTURY);
    // print_locale!(LOCALE_ICONSTRUCTEDLOCALE);
    // print_locale!(LOCALE_ICOUNTRY);
    // print_locale!(LOCALE_ICURRDIGITS);
    // print_locale!(LOCALE_ICURRENCY);
    // print_locale!(LOCALE_IDATE);
    // print_locale!(LOCALE_IDAYLZERO);
    // print_locale!(LOCALE_IDEFAULTANSICODEPAGE);
    // print_locale!(LOCALE_IDEFAULTCODEPAGE);
    // print_locale!(LOCALE_IDEFAULTCOUNTRY);
    // print_locale!(LOCALE_IDEFAULTEBCDICCODEPAGE);
    // print_locale!(LOCALE_IDEFAULTLANGUAGE);
    // print_locale!(LOCALE_IDEFAULTMACCODEPAGE);
    // print_locale!(LOCALE_IDIALINGCODE);
    // print_locale!(LOCALE_IDIGITS);
    // print_locale!(LOCALE_IDIGITSUBSTITUTION);
    // print_locale!(LOCALE_IFIRSTDAYOFWEEK);
    // print_locale!(LOCALE_IFIRSTWEEKOFYEAR);
    // print_locale!(LOCALE_IGEOID);
    // print_locale!(LOCALE_IINTLCURRDIGITS);
    // print_locale!(LOCALE_ILANGUAGE);
    // print_locale!(LOCALE_ILDATE);
    // print_locale!(LOCALE_ILZERO);
    // print_locale!(LOCALE_IMEASURE);
    // print_locale!(LOCALE_IMONLZERO);
    // print_locale!(LOCALE_INEGATIVEPERCENT);
    // print_locale!(LOCALE_INEGCURR);
    // print_locale!(LOCALE_INEGNUMBER);
    // print_locale!(LOCALE_INEGSEPBYSPACE);
    // print_locale!(LOCALE_INEGSIGNPOSN);
    // print_locale!(LOCALE_INEGSYMPRECEDES);
    // print_locale!(LOCALE_INEUTRAL);
    // print_locale!(LOCALE_IOPTIONALCALENDAR);
    // print_locale!(LOCALE_IPAPERSIZE);
    // print_locale!(LOCALE_IPOSITIVEPERCENT);
    // print_locale!(LOCALE_IPOSSEPBYSPACE);
    // print_locale!(LOCALE_IPOSSIGNPOSN);
    // print_locale!(LOCALE_IPOSSYMPRECEDES);
    // print_locale!(LOCALE_IREADINGLAYOUT);
    // print_locale!(LOCALE_ITIME);
    // print_locale!(LOCALE_ITIMEMARKPOSN);
    // print_locale!(LOCALE_ITLZERO);
    print_locale!(LOCALE_IUSEUTF8LEGACYACP);
    print_locale!(LOCALE_IUSEUTF8LEGACYOEMCP);
    // print_locale!(LOCALE_NAME_INVARIANT); // Expected u32, found PCWSTR
    // print_locale!(LOCALE_NAME_SYSTEM_DEFAULT); // Expected u32, found PCWSTR
    // print_locale!(LOCALE_NEUTRALDATA);
    // print_locale!(LOCALE_NOUSEROVERRIDE);
    print_locale!(LOCALE_REPLACEMENT);
    // print_locale!(LOCALE_RETURN_GENITIVE_NAMES);
    // print_locale!(LOCALE_RETURN_NUMBER);
    // print_locale!(LOCALE_S1159);
    // print_locale!(LOCALE_S2359);
    // print_locale!(LOCALE_SABBREVCTRYNAME);
    // print_locale!(LOCALE_SABBREVDAYNAME1);
    // print_locale!(LOCALE_SABBREVDAYNAME2);
    // print_locale!(LOCALE_SABBREVDAYNAME3);
    // print_locale!(LOCALE_SABBREVDAYNAME4);
    // print_locale!(LOCALE_SABBREVDAYNAME5);
    // print_locale!(LOCALE_SABBREVDAYNAME6);
    // print_locale!(LOCALE_SABBREVDAYNAME7);
    // print_locale!(LOCALE_SABBREVLANGNAME);
    // print_locale!(LOCALE_SABBREVMONTHNAME1);
    // print_locale!(LOCALE_SABBREVMONTHNAME10);
    // print_locale!(LOCALE_SABBREVMONTHNAME11);
    // print_locale!(LOCALE_SABBREVMONTHNAME12);
    // print_locale!(LOCALE_SABBREVMONTHNAME13);
    // print_locale!(LOCALE_SABBREVMONTHNAME2);
    // print_locale!(LOCALE_SABBREVMONTHNAME3);
    // print_locale!(LOCALE_SABBREVMONTHNAME4);
    // print_locale!(LOCALE_SABBREVMONTHNAME5);
    // print_locale!(LOCALE_SABBREVMONTHNAME6);
    // print_locale!(LOCALE_SABBREVMONTHNAME7);
    // print_locale!(LOCALE_SABBREVMONTHNAME8);
    // print_locale!(LOCALE_SABBREVMONTHNAME9);
    // print_locale!(LOCALE_SAM);
    print_locale!(LOCALE_SCONSOLEFALLBACKNAME);
    print_locale!(LOCALE_SCOUNTRY);
    // print_locale!(LOCALE_SCURRENCY);
    // print_locale!(LOCALE_SDATE);
    // print_locale!(LOCALE_SDAYNAME1);
    // print_locale!(LOCALE_SDAYNAME2);
    // print_locale!(LOCALE_SDAYNAME3);
    // print_locale!(LOCALE_SDAYNAME4);
    // print_locale!(LOCALE_SDAYNAME5);
    // print_locale!(LOCALE_SDAYNAME6);
    // print_locale!(LOCALE_SDAYNAME7);
    // print_locale!(LOCALE_SDECIMAL);
    // print_locale!(LOCALE_SDURATION);
    print_locale!(LOCALE_SENGCOUNTRY);
    print_locale!(LOCALE_SENGCURRNAME);
    print_locale!(LOCALE_SENGLANGUAGE);
    print_locale!(LOCALE_SENGLISHCOUNTRYNAME);
    print_locale!(LOCALE_SENGLISHDISPLAYNAME);
    print_locale!(LOCALE_SENGLISHLANGUAGENAME);
    // print_locale!(LOCALE_SGROUPING);
    print_locale!(LOCALE_SINTLSYMBOL);
    print_locale!(LOCALE_SISO3166CTRYNAME);
    print_locale!(LOCALE_SISO3166CTRYNAME2);
    print_locale!(LOCALE_SISO639LANGNAME);
    print_locale!(LOCALE_SISO639LANGNAME2);
    print_locale!(LOCALE_SKEYBOARDSTOINSTALL);
    print_locale!(LOCALE_SLANGDISPLAYNAME);
    print_locale!(LOCALE_SLANGUAGE);
    // print_locale!(LOCALE_SLIST);
    print_locale!(LOCALE_SLOCALIZEDCOUNTRYNAME);
    print_locale!(LOCALE_SLOCALIZEDDISPLAYNAME);
    print_locale!(LOCALE_SLOCALIZEDLANGUAGENAME);
    // print_locale!(LOCALE_SLONGDATE);
    // print_locale!(LOCALE_SMONDECIMALSEP);
    // print_locale!(LOCALE_SMONGROUPING);
    // print_locale!(LOCALE_SMONTHDAY);
    // print_locale!(LOCALE_SMONTHNAME1);
    // print_locale!(LOCALE_SMONTHNAME10);
    // print_locale!(LOCALE_SMONTHNAME11);
    // print_locale!(LOCALE_SMONTHNAME12);
    // print_locale!(LOCALE_SMONTHNAME13);
    // print_locale!(LOCALE_SMONTHNAME2);
    // print_locale!(LOCALE_SMONTHNAME3);
    // print_locale!(LOCALE_SMONTHNAME4);
    // print_locale!(LOCALE_SMONTHNAME5);
    // print_locale!(LOCALE_SMONTHNAME6);
    // print_locale!(LOCALE_SMONTHNAME7);
    // print_locale!(LOCALE_SMONTHNAME8);
    // print_locale!(LOCALE_SMONTHNAME9);
    // print_locale!(LOCALE_SMONTHOUSANDSEP);
    // print_locale!(LOCALE_SNAME);
    // print_locale!(LOCALE_SNAN);
    print_locale!(LOCALE_SNATIVECOUNTRYNAME);
    print_locale!(LOCALE_SNATIVECTRYNAME);
    print_locale!(LOCALE_SNATIVECURRNAME);
    // print_locale!(LOCALE_SNATIVEDIGITS);
    print_locale!(LOCALE_SNATIVEDISPLAYNAME);
    print_locale!(LOCALE_SNATIVELANGNAME);
    print_locale!(LOCALE_SNATIVELANGUAGENAME);
    // print_locale!(LOCALE_SNEGATIVESIGN);
    // print_locale!(LOCALE_SNEGINFINITY);
    // print_locale!(LOCALE_SOPENTYPELANGUAGETAG);
    // print_locale!(LOCALE_SPARENT); // Not found in this windows crate version/module
    // print_locale!(LOCALE_SPECIFICDATA);
    // print_locale!(LOCALE_SPERCENT);
    // print_locale!(LOCALE_SPERMILLE);
    // print_locale!(LOCALE_SPM);
    // print_locale!(LOCALE_SPOSINFINITY);
    // print_locale!(LOCALE_SPOSITIVESIGN);
    // print_locale!(LOCALE_SRELATIVELONGDATE);
    // print_locale!(LOCALE_SSCRIPTS);
    // print_locale!(LOCALE_SSHORTDATE);
    // print_locale!(LOCALE_SSHORTESTAM);
    // print_locale!(LOCALE_SSHORTESTDAYNAME1);
    // print_locale!(LOCALE_SSHORTESTDAYNAME2);
    // print_locale!(LOCALE_SSHORTESTDAYNAME3);
    // print_locale!(LOCALE_SSHORTESTDAYNAME4);
    // print_locale!(LOCALE_SSHORTESTDAYNAME5);
    // print_locale!(LOCALE_SSHORTESTDAYNAME6);
    // print_locale!(LOCALE_SSHORTESTDAYNAME7);
    // print_locale!(LOCALE_SSHORTESTPM);
    // print_locale!(LOCALE_SSHORTTIME);
    // print_locale!(LOCALE_SSORTLOCALE);
    // print_locale!(LOCALE_SSORTNAME);
    // print_locale!(LOCALE_STHOUSAND);
    // print_locale!(LOCALE_STIME);
    // print_locale!(LOCALE_STIMEFORMAT);
    print_locale!(LOCALE_SUPPLEMENTAL);
    // print_locale!(LOCALE_SYEARMONTH);
    // print_locale!(LOCALE_SYSTEM_DEFAULT);
    // print_locale!(LOCALE_USER_DEFAULT);
    print_locale!(LOCALE_USE_CP_ACP);
    print_locale!(LOCALE_WINDOWS);
    print_locale!(LOWLEVEL_SERVICE_TYPES);
    // print_locale!(LOW_SURROGATE_END);
    // print_locale!(LOW_SURROGATE_START);
}
