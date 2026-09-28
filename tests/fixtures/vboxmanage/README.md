# VBoxManage output fixtures

Sanitized samples of `VBoxManage` output used by the parser tests in
`crates/vmsa-core/src/vbox/parse.rs`.

Provenance: the line formats were reconstructed from the VirtualBox 7.2 source
(`src/VBox/Frontends/VBoxManage/VBoxManageInfo.cpp`, `VBoxManageList.cpp`,
`VBoxManageGuestProp.cpp`, `VBoxManageMisc.cpp`) and from output observed on a
VirtualBox 7.2 host. Paths, UUIDs and user names are synthetic. When you capture
real output from a machine, replace these files with the sanitized capture and
keep the same file names.
