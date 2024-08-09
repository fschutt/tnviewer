use crate::ui::Aenderungen;
use crate::SplitNasXml;

pub fn aenderungen_zu_fa_xml(aenderungen: &Aenderungen, split_nas: &SplitNasXml) -> String {
    format!(
        include_str!("./antrag.xml"),
        crs = "",
        wfs_delete = "",
        wfs_replace = "",
        wfs_insert = "",
        profilkennung = "",
        antragsnr = ""
    )
}

