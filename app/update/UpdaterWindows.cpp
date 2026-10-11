// SPDX-License-Identifier: MIT
// mitcad-updater.exe (mitcad#9, Windows): installs an update once Mitcad
// has quit. Mitcad copies it next to the verified installer in the user's
// temp folder (the installer replaces bin, where it comes from) and starts
// it as it ends (app/update/UpdateInstaller.hpp):
//
//   mitcad-updater.exe --wait <pid> --installer <file> --dir <installation>
//                      --scope user|all --restart <mitcad.exe> --log <file>
//
// It waits for that process to end, runs the installer silently into the
// installation folder and its scope (mitcad#118): an installation just for
// the current user with /S /CurrentUser /D=<dir>, as the user, so without a
// UAC prompt; one for all users (Program Files) with /S /AllUsers /D=<dir>,
// elevated (verb runas, UAC asks) unless this process has administrator
// rights already. Then it starts Mitcad again, also when the installer
// failed or the elevation was refused: Mitcad then tells so from the log's
// last line. Win32 only, with the C++ runtime linked statically, so that it
// runs on its own in the temp folder; never elevated itself, so Mitcad
// starts again as the user.

#include <windows.h>

#include <shellapi.h>

#include <cwchar>
#include <string>
#include <vector>

namespace {

std::wstring g_log;

// Appends a line to the log (UTF-8).
void note(const std::wstring& line) {
  if (g_log.empty()) {
    return;
  }
  const std::wstring text = line + L"\r\n";
  const int size = WideCharToMultiByte(CP_UTF8, 0, text.c_str(), static_cast<int>(text.size()), nullptr, 0,
                                       nullptr, nullptr);
  std::string utf8(static_cast<std::size_t>(size), '\0');
  WideCharToMultiByte(CP_UTF8, 0, text.c_str(), static_cast<int>(text.size()), utf8.data(), size, nullptr,
                      nullptr);
  const HANDLE file = CreateFileW(g_log.c_str(), FILE_APPEND_DATA, FILE_SHARE_READ, nullptr, OPEN_ALWAYS,
                                  FILE_ATTRIBUTE_NORMAL, nullptr);
  if (file != INVALID_HANDLE_VALUE) {
    DWORD written = 0;
    WriteFile(file, utf8.data(), static_cast<DWORD>(utf8.size()), &written, nullptr);
    CloseHandle(file);
  }
}

std::wstring option(const std::vector<std::wstring>& arguments, const wchar_t* name) {
  for (std::size_t i = 1; i + 1 < arguments.size(); ++i) {
    if (arguments[i] == name) {
      return arguments[i + 1];
    }
  }
  return std::wstring();
}

// Whether this process has administrator rights: the Administrators group
// is enabled in its token (an elevated one, or no UAC), as the installer's
// own check (UserInfo::GetAccountType) sees it.
bool isAdministrator() {
  SID_IDENTIFIER_AUTHORITY authority = SECURITY_NT_AUTHORITY;
  PSID administrators = nullptr;
  if (!AllocateAndInitializeSid(&authority, 2, SECURITY_BUILTIN_DOMAIN_RID, DOMAIN_ALIAS_RID_ADMINS, 0, 0, 0, 0, 0,
                                0, &administrators)) {
    return false;
  }
  BOOL member = FALSE;
  if (!CheckTokenMembership(nullptr, administrators, &member)) {
    member = FALSE;
  }
  FreeSid(administrators);
  return member != FALSE;
}

// Runs the installer and waits for it; its exit code, or nothing when it
// did not start. `scope`: "user", "all" or empty (the installer decides,
// as for an updater of an earlier release).
bool runInstaller(const std::wstring& installer, const std::wstring& dir, const std::wstring& scope,
                  DWORD& exitCode) {
  const bool allUsers = scope == L"all";
  const bool elevate = allUsers && !isAdministrator();
  // NSIS: /D must come last and without quotes, also with spaces.
  std::wstring parameters = L"/S ";
  if (allUsers) {
    parameters += L"/AllUsers ";
  } else if (scope == L"user") {
    parameters += L"/CurrentUser ";
  }
  parameters += L"/D=" + dir;
  note(L"Running " + installer + L" " + parameters +
       (elevate ? std::wstring(L" (for all users, asking for administrator rights)")
                : allUsers ? std::wstring(L" (for all users, with administrator rights)")
                           : std::wstring(L" (as the user)")));
  SHELLEXECUTEINFOW info = {};
  info.cbSize = sizeof(info);
  info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
  info.lpVerb = elevate ? L"runas" : L"open";
  info.lpFile = installer.c_str();
  info.lpParameters = parameters.c_str();
  info.nShow = SW_SHOWNORMAL;
  if (!ShellExecuteExW(&info) || info.hProcess == nullptr) {
    const DWORD error = GetLastError();
    note(error == ERROR_CANCELLED ? std::wstring(L"The installer was not allowed to run (elevation refused)")
                                 : L"The installer could not be started (error " + std::to_wstring(error) + L")");
    return false;
  }
  WaitForSingleObject(info.hProcess, INFINITE);
  GetExitCodeProcess(info.hProcess, &exitCode);
  CloseHandle(info.hProcess);
  note(exitCode == 0 ? std::wstring(L"The installer finished")
                    : L"The installer failed (exit code " + std::to_wstring(exitCode) + L")");
  return true;
}

} // namespace

int WINAPI wWinMain(HINSTANCE, HINSTANCE, PWSTR, int) {
  int count = 0;
  LPWSTR* argv = CommandLineToArgvW(GetCommandLineW(), &count);
  std::vector<std::wstring> arguments;
  for (int i = 0; argv != nullptr && i < count; ++i) {
    arguments.emplace_back(argv[i]);
  }
  LocalFree(argv);
  g_log = option(arguments, L"--log");
  const std::wstring pid = option(arguments, L"--wait");
  const std::wstring installer = option(arguments, L"--installer");
  const std::wstring dir = option(arguments, L"--dir");
  const std::wstring restart = option(arguments, L"--restart");
  const std::wstring scope = option(arguments, L"--scope");
  if (installer.empty() || dir.empty() || restart.empty() || (!scope.empty() && scope != L"user" && scope != L"all")) {
    note(L"Usage: mitcad-updater --wait <pid> --installer <file> --dir <folder> --scope user|all --restart <exe> "
         L"--log <file>");
    return 2;
  }

  if (!pid.empty()) {
    note(L"Waiting for Mitcad (process " + pid + L") to end");
    const HANDLE process = OpenProcess(SYNCHRONIZE, FALSE, static_cast<DWORD>(std::wcstoul(pid.c_str(), nullptr, 10)));
    if (process != nullptr) {
      const DWORD waited = WaitForSingleObject(process, 120000);
      CloseHandle(process);
      if (waited != WAIT_OBJECT_0) {
        note(L"Mitcad did not end within two minutes, so the update was not installed");
        return 1;
      }
    }
  }

  DWORD exitCode = 1;
  runInstaller(installer, dir, scope, exitCode);

  STARTUPINFOW startup = {};
  startup.cb = sizeof(startup);
  PROCESS_INFORMATION started = {};
  std::wstring command = L"\"" + restart + L"\"";
  if (CreateProcessW(restart.c_str(), command.data(), nullptr, nullptr, FALSE, 0, nullptr, dir.c_str(), &startup,
                     &started)) {
    CloseHandle(started.hThread);
    CloseHandle(started.hProcess);
    note(L"Started " + restart);
  } else {
    note(L"Could not start " + restart + L" (error " + std::to_wstring(GetLastError()) + L")");
  }
  return exitCode == 0 ? 0 : 1;
}
