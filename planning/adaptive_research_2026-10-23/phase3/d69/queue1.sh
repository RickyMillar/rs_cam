#!/usr/bin/env bash
cd /home/ricky/personal_repos/rs_cam/planning/adaptive_research_2026-10-23/phase3/d69
./run_play.sh a0_corne $PWD/jobs/corne_play_base.toml
RS_CAM_D68_OPTION2=1 ./run_play.sh opt2_corne $PWD/jobs/corne_play_base.toml
RS_CAM_D68_OPTION2=1 RS_CAM_D69_MERGE=1 ./run_play.sh opt2m_corne $PWD/jobs/corne_play_base.toml
./run_play.sh a0_wanaka $PWD/jobs/wanaka_play_base.toml --resolution 0.6
RS_CAM_D68_OPTION2=1 ./run_play.sh opt2_wanaka $PWD/jobs/wanaka_play_base.toml --resolution 0.6
RS_CAM_D68_OPTION2=1 RS_CAM_D69_MERGE=1 ./run_play.sh opt2m_wanaka $PWD/jobs/wanaka_play_base.toml --resolution 0.6
echo QUEUE1 DONE
