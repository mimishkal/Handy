import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useMeetingStore } from "@/stores/meetingStore";
import { ArrowLeft, Mic, Users, Copy, Check } from "lucide-react";
import type { MeetingSegment } from "@/bindings";

interface Props {
  meetingId: string;
  title: string;
  onBack: () => void;
}

export const MeetingTranscriptViewer: React.FC<Props> = ({
  meetingId,
  title,
  onBack,
}) => {
  const { t } = useTranslation();
  const loadTranscript = useMeetingStore((s) => s.loadTranscript);
  const [segments, setSegments] = useState<MeetingSegment[]>([]);
  const [loading, setLoading] = useState(true);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    setLoading(true);
    loadTranscript(meetingId).then((data) => {
      setSegments(data);
      setLoading(false);
    });
  }, [meetingId, loadTranscript]);

  const handleCopyAll = async () => {
    const text = segments
      .map(
        (s) =>
          `[${s.speaker === "me" ? t("meeting.speaker.me") : t("meeting.speaker.others")}] ${s.transcription_text}`,
      )
      .join("\n");
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="space-y-2">
      <div className="px-4 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <button
            onClick={onBack}
            className="p-1 hover:bg-mid-gray/20 rounded transition-colors cursor-pointer"
          >
            <ArrowLeft className="w-4 h-4" />
          </button>
          <h2 className="text-xs font-medium text-mid-gray uppercase tracking-wide truncate">
            {title}
          </h2>
        </div>
        <button
          onClick={handleCopyAll}
          className="flex items-center gap-1 text-xs text-text/50 hover:text-text/80 transition-colors cursor-pointer"
        >
          {copied ? (
            <Check className="w-3.5 h-3.5" />
          ) : (
            <Copy className="w-3.5 h-3.5" />
          )}
          {t("meeting.transcript.copyAll")}
        </button>
      </div>
      <div className="bg-background border border-mid-gray/20 rounded-lg overflow-hidden">
        {loading ? (
          <div className="px-4 py-3 text-center text-text/60 text-sm">
            {t("meeting.transcript.loading")}
          </div>
        ) : segments.length === 0 ? (
          <div className="px-4 py-3 text-center text-text/60 text-sm">
            {t("meeting.transcript.empty")}
          </div>
        ) : (
          <div className="divide-y divide-mid-gray/10 max-h-[500px] overflow-y-auto">
            {segments.map((seg) => (
              <div key={seg.id} className="px-4 py-2 flex gap-2">
                <div className="shrink-0 pt-0.5">
                  {seg.speaker === "me" ? (
                    <Mic className="w-3.5 h-3.5 text-blue-400" />
                  ) : (
                    <Users className="w-3.5 h-3.5 text-green-400" />
                  )}
                </div>
                <p className="text-sm text-text/90 select-text cursor-text">
                  {seg.transcription_text}
                </p>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};
