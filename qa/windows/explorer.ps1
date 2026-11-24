$shell = New-Object -ComObject Shell.Application
for ($attempt = 0; $attempt -lt 40; $attempt++) {
    foreach ($window in $shell.Windows()) {
        if ($window.Document.Folder.Self.Path -ne [IO.Path]::GetDirectoryName($env:JOT_QA_DOCUMENT)) { continue }
        if ($window.Document.SelectedItems().Item(0).Path -eq $env:JOT_QA_DOCUMENT) {
            $window.Quit()
            exit 0
        }
    }
    Start-Sleep -Milliseconds 250
}
exit 1
