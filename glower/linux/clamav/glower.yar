rule Linux_Rootkit_LKM : rootkit linux
{
    meta:
        severity = "malware"
    strings:
        $m1 = "MODULE_LICENSE"
        $m2 = "vermagic="
        $k1 = "sys_call_table"
        $k2 = "kallsyms_lookup_name"
        $k3 = "set_memory_rw"
        $k4 = "write_cr0"
        $h1 = "hide"
        $h2 = "hook"
        $h3 = "getdents"
        $h4 = "tcp4_seq_show"
    condition:
        uint32(0) == 0x464C457F and (any of ($m*)) and
        ((2 of ($k*)) or ((any of ($k*)) and (2 of ($h*))))
}

rule Linux_Suspicious_Setuid_Shell : privesc linux
{
    meta:
        severity = "malware"
    strings:
        $s1 = "chmod +s"
        $s2 = "chmod 4755"
        $s3 = "chmod u+s"
        $t1 = "/bin/bash"
        $t2 = "/bin/sh"
        $t3 = "/bin/dash"
        $c1 = "cp /bin/"
        $c2 = "setuid(0)"
        $c3 = "setresuid"
    condition:
        ((any of ($s*)) and (any of ($t*))) or ((any of ($c*)) and (any of ($s*)))
}

rule Win_ProcessHollowing : injection windows
{
    meta:
        severity = "malware"
    strings:
        $a = "NtUnmapViewOfSection"
        $b = "ZwUnmapViewOfSection"
        $c = "CreateProcessA"
        $d = "CreateProcessW"
        $e = "WriteProcessMemory"
        $f = "SetThreadContext"
        $g = "GetThreadContext"
        $h = "ResumeThread"
    condition:
        uint16(0) == 0x5A4D and (any of ($a,$b)) and (any of ($c,$d)) and
        $e and (any of ($f,$g)) and $h
}

rule Win_Credential_Dump_Tools : stealer windows
{
    meta:
        severity = "malware"
    strings:
        $m1 = "sekurlsa::" nocase
        $m2 = "lsadump::" nocase
        $m3 = "kerberos::" nocase
        $m4 = "privilege::debug" nocase
        $m5 = "gentilkiwi" nocase
        $l1 = "lsass.exe" nocase
        $l2 = "MiniDumpWriteDump" nocase
        $l3 = "comsvcs.dll, MiniDump" nocase
        $s1 = "SAM\\SAM\\Domains\\Account" nocase
        $s2 = "reg save hklm\\sam" nocase
    condition:
        (any of ($m*)) or ($l1 and (any of ($l2,$l3))) or (any of ($s*))
}

rule Win_Ransomware_Behaviour : ransomware windows
{
    meta:
        severity = "malware"
    strings:
        $e1 = "CryptEncrypt"
        $e2 = "BCryptEncrypt"
        $e3 = "CryptGenKey"
        $e4 = "CryptImportKey"
        $f1 = "FindFirstFileW"
        $f2 = "FindNextFileW"
        $f3 = "GetLogicalDriveStringsW"
        $d1 = "vssadmin" nocase
        $d2 = "bcdedit" nocase
        $d3 = "wbadmin" nocase
        $n1 = "README" nocase
        $n2 = "DECRYPT" nocase
        $n3 = "HOW_TO" nocase
        $n4 = ".onion" nocase
    condition:
        uint16(0) == 0x5A4D and (any of ($e*)) and (2 of ($f*)) and
        ((any of ($d*)) or (2 of ($n*)))
}

rule Office_Remote_Template_Injection : macro document
{
    meta:
        severity = "malware"
    strings:
        $r1 = "attachedTemplate" nocase
        $r2 = "TargetMode=\"External\"" nocase
        $u1 = "http://"
        $u2 = "https://"
        $u3 = "file://"
        $u4 = "\\\\"
    condition:
        $r1 and $r2 and (any of ($u*))
}

rule Script_Dropper_WSH : dropper windows
{
    meta:
        severity = "malware"
    strings:
        $o1 = "ActiveXObject" nocase
        $o2 = "CreateObject" nocase
        $s1 = "Scripting.FileSystemObject" nocase
        $s2 = "ADODB.Stream" nocase
        $s3 = "MSXML2.XMLHTTP" nocase
        $s4 = "WScript.Shell" nocase
        $r1 = ".Run" nocase
        $r2 = ".Exec" nocase
        $r3 = "SaveToFile" nocase
    condition:
        (any of ($o*)) and (2 of ($s*)) and (any of ($r*))
}

rule LNK_Suspicious_Command : dropper windows
{
    meta:
        severity = "malware"
    strings:
        $lnk = { 4C 00 00 00 01 14 02 00 }
        $c1 = "powershell" nocase wide ascii
        $c2 = "cmd.exe" nocase wide ascii
        $c3 = "mshta" nocase wide ascii
        $c4 = "rundll32" nocase wide ascii
        $c5 = "wscript" nocase wide ascii
        $c6 = "-EncodedCommand" nocase wide ascii
    condition:
        $lnk at 0 and any of ($c*)
}
