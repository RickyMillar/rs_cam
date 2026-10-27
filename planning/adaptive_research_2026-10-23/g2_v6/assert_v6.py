"""G2 v6: assert every claim the page makes, from numbers_v6.json."""
import json, os

H = os.path.dirname(os.path.abspath(__file__))
N = json.load(open(os.path.join(H, "numbers_v6.json")))
C, W = N["corne"], N["wanaka"]
ok = 0


def claim(cond, text):
    global ok
    assert cond, text
    ok += 1
    print("ok:", text)


cp, g = C["cp"], C["geo_s2p1"]
claim(W["geo_s2p0"]["job"] < W["cp"]["job"] * 0.6, "wanaka geodesic 2.0 mm job is under 0.6x CP")
claim(W["load_s2p0"]["L_max"] < W["load_cp"]["L_max"] / 3, "wanaka geodesic 2.0 mm peak load is under a third of CP's")
claim(W["geo_s3p5"]["job"] * 2.5 < W["cp"]["job"], "wanaka geodesic 3.5 mm is about three times faster than CP")
claim(abs(W["geo_s3p5"]["op1"] + W["geo_s3p5"]["edge"] - W["geo_s3p5"]["job"]) < 1.0, "wanaka 3.5 mm job = op 1 + edge op")
claim(W["left_s3p5"]["area_over_0p6_mm2"] > 1.9 * W["left_cp"]["area_over_0p6_mm2"], "wanaka 3.5 mm leaves about twice CP's area over 0.6 mm")
claim(W["left_s2p0"]["area_over_0p6_mm2"] < 1.3 * W["left_cp"]["area_over_0p6_mm2"], "wanaka 2.0 mm leaves about what CP leaves")
claim(W["load_cp"]["L_max"] > 2.5, "CP reaches 2.62 on wanaka")
r = (g["op1"] + g["edge"]) / cp["op1"]
claim(1.0 < r < 1.1, f"Corne 6 mm roughing is x{r:.2f} of CP op 1")
claim(C["load_geo"]["L_p99"] < 0.55 * C["load_cp"]["L_p99"], "Corne geodesic p99 load is about half of CP's")
claim(g["job"] > cp["job"], "the whole Corne job is still slower than CP")
claim(g["rest3"] - cp["rest3"] > 0.8 * (g["job"] - cp["job"] - (g["op1"] + g["edge"] - cp["op1"])), "the 3 mm rest op makes the rest of the Corne gap")
claim(abs(cp["op1"] + cp["rest3"] - cp["job"]) < 1.0, "Corne CP job = op 1 + 3 mm rest")
claim(abs(g["op1"] + g["edge"] + g["rest3"] - g["job"]) < 1.0, "Corne geodesic job = op 1 + edge + 3 mm rest")
print(f"{ok} claims hold")
