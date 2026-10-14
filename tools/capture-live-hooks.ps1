param([Parameter(Mandatory=$true)][string]$NativeRuntime,[Parameter(Mandatory=$true)][string]$CaptureDir,[int]$Seconds=240)
$ErrorActionPreference='Stop'
$native=Get-Content -LiteralPath $NativeRuntime -Encoding UTF8 -Raw|ConvertFrom-Json
New-Item -ItemType Directory -Path $CaptureDir -Force|Out-Null
$listener=[Net.HttpListener]::new();$listener.Prefixes.Add('http://127.0.0.1:47941/');$listener.Start()
$r=@{v=2;port=47941;token=$native.token;pid=$PID;appVersion='capture';instanceId=[guid]::NewGuid().ToString();startedAt=0}
[IO.File]::WriteAllText((Join-Path $CaptureDir 'runtime.json'),($r|ConvertTo-Json -Compress),[Text.UTF8Encoding]::new($false))
$end=[DateTime]::UtcNow.AddSeconds($Seconds);$request=$listener.GetContextAsync();$count=0
try{
    while([DateTime]::UtcNow -lt $end){
        if(-not $request.Wait(1000)){continue};$context=$request.Result
        if($context.Request.Headers['Authorization'] -ne ('Bearer '+$native.token) -or $context.Request.HttpMethod -ne 'POST' -or $context.Request.RawUrl -ne '/v2/event'){
            $context.Response.StatusCode=401;$context.Response.Close();$request=$listener.GetContextAsync();continue
        }
        $reader=[IO.StreamReader]::new($context.Request.InputStream,[Text.Encoding]::UTF8);$body=$reader.ReadToEnd();$reader.Dispose()
        $envelope=$body|ConvertFrom-Json;$payload=$envelope.payload|ConvertFrom-Json
        $sample=@{capturedFromRealSession=$true;receivedAt=[DateTime]::UtcNow.ToString('o');agent=$envelope.agent;event=$envelope.event;src=$envelope.src;payload=@{hook_event_name=$payload.hook_event_name;session_id=$payload.session_id;prompt_id=$payload.prompt_id;turn_id=$payload.turn_id;agent_id=$payload.agent_id;tool_use_id=$payload.tool_use_id;tool_name=$payload.tool_name;hasToolInput=($null -ne $payload.tool_input);hasToolResponse=($null -ne $payload.tool_response)};redactedFields=@('prompt','cwd','transcript_path','tool_input','tool_response','message','last_assistant_message')}
        [IO.File]::AppendAllText((Join-Path $CaptureDir 'events.jsonl'),(($sample|ConvertTo-Json -Depth 5 -Compress)+[Environment]::NewLine),[Text.UTF8Encoding]::new($false));$count++
        $context.Response.StatusCode=202;$context.Response.Close()
        try{Invoke-RestMethod -Uri ('http://127.0.0.1:'+$native.port+'/v2/event') -Method Post -Headers @{Authorization='Bearer '+$native.token} -ContentType 'application/json' -Body ([Text.Encoding]::UTF8.GetBytes($body)) -TimeoutSec 2|Out-Null}catch{}
        $request=$listener.GetContextAsync()
    }
}finally{$listener.Stop();$listener.Close()}
Write-Output ('Captured '+$count+' redacted real hook events')
