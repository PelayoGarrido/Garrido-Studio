from pathlib import Path
import hashlib
import json
import math
import subprocess
import tempfile
import wave

ROOT=Path(__file__).resolve().parents[1]
CLI=ROOT/"target/release/garrido-studio"


def run(*args):
    result=subprocess.run(list(map(str,args)),capture_output=True,text=True,timeout=30)
    if result.returncode:
        raise RuntimeError(result.stderr)


def pcm(path):
    with wave.open(str(path),"rb") as stream:
        return stream.getparams()[:4],stream.readframes(stream.getnframes())


def main():
    with tempfile.TemporaryDirectory(prefix="audio-smoke-",dir=ROOT/"outputs") as directory:
        folder=Path(directory)
        source=folder/"source.wav"
        raw=bytearray()
        for i in range(12345):
            for value in (math.sin(i*math.tau*317/48000)*0.32,math.sin(i*math.tau*659/48000)*0.17+(i%17-8)*0.0001):
                raw.extend(round(value*8388608).to_bytes(3,"little",signed=True))
        with wave.open(str(source),"wb") as stream:
            stream.setparams((2,3,48000,0,"NONE","not compressed"));stream.writeframes(raw)
        own=folder/"own.flac";apple_decoded=folder/"apple-decoded.wav"
        run(CLI,"convert-audio",source,own)
        assert own.read_bytes()[26:42] == hashlib.md5(raw).digest()
        run("/usr/bin/afconvert","-f","WAVE","-d","LEI24",own,apple_decoded)
        assert pcm(source)==pcm(apple_decoded)
        apple=folder/"apple.flac";own_decoded=folder/"own-decoded.wav"
        run("/usr/bin/afconvert","-f","flac","-d","flac",source,apple)
        run(CLI,"convert-audio",apple,own_decoded)
        apple_digest = apple.read_bytes()[26:42]
        if apple_digest != bytes(16):
            assert apple_digest == hashlib.md5(raw).digest()
        assert pcm(source)==pcm(own_decoded)
        own_aiff=folder/"own.aiff";apple_aiff_decoded=folder/"apple-aiff-decoded.wav"
        run(CLI,"convert-audio",source,own_aiff)
        run("/usr/bin/afconvert","-f","WAVE","-d","LEI24",own_aiff,apple_aiff_decoded)
        assert pcm(source)==pcm(apple_aiff_decoded)
        for encoding,kind in (("BEI24","AIFF"),("BEI24","AIFC"),("BEF32","AIFC"),("BEF64","AIFC")):
            apple_aiff=folder/(kind+"-"+encoding+".aifc")
            own_aiff_decoded=folder/(kind+"-"+encoding+"-decoded.wav")
            run("/usr/bin/afconvert","-f",kind,"-d",encoding,source,apple_aiff)
            run(CLI,"convert-audio",apple_aiff,own_aiff_decoded)
            assert pcm(source)==pcm(own_aiff_decoded)
        report={"frames":12345,"rate":48000,"depth":24,"channels":2,
                "own_flac_decoded_by_apple_pcm_exact":True,"apple_flac_decoded_by_own_pcm_exact":True,
                "pcm_bytes":len(raw),"own_flac_bytes":own.stat().st_size,"apple_flac_bytes":apple.stat().st_size,
                "own_pcm_digest_matches_independent_hashlib":True,
                "apple_pcm_digest_supplied":apple_digest != bytes(16),
                "own_aiff_decoded_by_apple_pcm_exact":True,
                "apple_aiff_aifc_integer_and_float_decoded_by_own_pcm_exact":True,
                "cli_sha256":hashlib.sha256(CLI.read_bytes()).hexdigest(),
                "audio_played":False}
    (ROOT/"outputs/audio-smoke.json").write_text(json.dumps(report,indent=2)+"\n")
    print(json.dumps(report,indent=2))


if __name__=="__main__":
    main()
