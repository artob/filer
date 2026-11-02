@echo off
rem This is free and unencumbered software released into the public domain.
rem
rem Registers the job runner: at each logon, elevated, it runs jobs
rem dropped into the shared folder.
schtasks /create /tn FilerRunner /sc onlogon /ru Docker /it /rl highest /f /tr "powershell -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File C:\OEM\runner.ps1"
schtasks /run /tn FilerRunner
