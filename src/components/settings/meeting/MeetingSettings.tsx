import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useMeetingStore } from "@/stores/meetingStore";
import { commands } from "@/bindings";
import { MeetingHistory } from "./MeetingHistory";
import { Mic, Users, ShieldAlert } from "lucide-react";
import { platform } from "@tauri-apps/plugin-os";

const LiveTranscript: React.FC = () => {
  const { t } = useTranslation();
  const liveSegments = useMeetingStore((s) => s.liveSegments);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    scrollRef.current?.scrollTo({
      top: scrollRef.current.scrollHeight,
      behavior: "smooth",
    });
  }, [liveSegments]);

  if (liveSegments.length === 0) {
    return (
      <div className="px-4 py-8 text-center text-text/50 text-sm">
        {t("meeting.liveTranscript.waiting")}
      </div>
    );
  }

  return (
    <div
      ref={scrollRef}
      className="max-h-96 overflow-y-auto divide-y divide-mid-gray/10"
    >
      {liveSegments.map((seg, i) => (
        <div key={i} className="px-4 py-2 flex gap-2">
          <div className="shrink-0 pt-0.5">
            {seg.speaker === "me" ? (
              <Mic className="w-3.5 h-3.5 text-blue-400" />
            ) : (
              <Users className="w-3.5 h-3.5 text-green-400" />
            )}
          </div>
          <p className="text-sm text-text/90 select-text cursor-text">
            {seg.text}
          </p>
        </div>
      ))}
    </div>
  );
};

const ScreenRecordingWarning: React.FC = () => {
  const { t } = useTranslation();
  const [hasPermission, setHasPermission] = useState<boolean | null>(null);
  const isMac = platform() === "macos";

  useEffect(() => {
    if (!isMac) {
      setHasPermission(true);
      return;
    }
    commands.checkScreenRecordingPermission().then((granted) => {
      setHasPermission(granted);
    });
  }, [isMac]);

  if (hasPermission !== false) return null;

  return (
    <div className="bg-yellow-500/10 border border-yellow-500/30 rounded-lg px-4 py-3 flex gap-3 items-start">
      <ShieldAlert className="w-5 h-5 text-yellow-500 shrink-0 mt-0.5" />
      <div className="text-sm">
        <p className="font-medium text-yellow-200">
          {t("meeting.permissions.screenRecording.title")}
        </p>
        <p className="text-text/60 mt-1">
          {t("meeting.permissions.screenRecording.description")}
        </p>
      </div>
    </div>
  );
};

export const MeetingSettings: React.FC = () => {
  const { t } = useTranslation();
  const activeSessionId = useMeetingStore((s) => s.activeSessionId);
  const initialize = useMeetingStore((s) => s.initialize);

  useEffect(() => {
    initialize();
  }, [initialize]);

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      {/* Screen Recording permission warning */}
      <ScreenRecordingWarning />

      {/* Live meeting panel */}
      {activeSessionId && (
        <div className="space-y-2">
          <div className="px-4">
            <h2 className="text-xs font-medium text-mid-gray uppercase tracking-wide">
              {t("meeting.live.title")}
            </h2>
          </div>
          <div className="bg-background border border-green-500/30 rounded-lg overflow-hidden">
            <div className="px-4 py-2 border-b border-mid-gray/20 flex items-center gap-2">
              <span className="w-2 h-2 rounded-full bg-red-500 animate-pulse" />
              <span className="text-sm font-medium">
                {t("meeting.live.recording")}
              </span>
            </div>
            <LiveTranscript />
          </div>
        </div>
      )}

      {/* Meeting history */}
      <MeetingHistory />
    </div>
  );
};
