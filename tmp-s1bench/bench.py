import json, time, urllib.request

SYS = "You are a text normalizer for speech-to-text transcripts. The input begins with a control line specifying the styling, structure, and context settings; clean the transcript to match those settings and output only the cleaned text."

TRANSCRIPT = """okay so um here's what we need to do for the launch first uh we have to finalize the pricing page which should be like twenty nine dollars a month second um maria needs to record the demo video by thursday third we should email the beta users about the update and um their feedback has been mostly positive except for the export bug also can someone fix the export bug before friday because thats blocking the launch okay thats it thanks"""

def run(control):
    prompt = (f"<|im_start|>system\n{SYS}<|im_end|>\n<|im_start|>user\n"
              f"{control}\n{TRANSCRIPT}"
              f"<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n")
    body = json.dumps({"prompt": prompt, "n_predict": 500, "temperature": 0}).encode()
    req = urllib.request.Request("http://127.0.0.1:8912/completion", data=body,
                                 headers={"Content-Type": "application/json"})
    t0 = time.time()
    with urllib.request.urlopen(req, timeout=120) as r:
        res = json.loads(r.read())
    wall = time.time() - t0
    print(f"\n===== CONTROL: {control} =====")
    print(f"gen {res['timings']['predicted_per_second']:.1f} tok/s | wall {wall:.2f}s")
    print("--- raw output ---")
    print(res["content"])

for ctrl in [
    "[Styling: semi-formal] [Structure: bullets] [Context: work]",
    "[Styling: semi-formal] [Structure: markdown] [Context: work]",
    "[Styling: semi-formal] [Structure: prose] [Context: work]",
]:
    run(ctrl)
