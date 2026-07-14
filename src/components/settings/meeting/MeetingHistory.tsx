import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { useMeetingStore } from "@/stores/meetingStore";
import { MeetingTranscriptViewer } from "./MeetingTranscriptViewer";
import { Trash2, ChevronRight, Clock } from "lucide-react";
import type { MeetingSummary } from "@/bindings";

const formatDuration = (seconds: number | null): string => {
  if (!seconds) return "--:--";
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins}m ${secs}s`;
};

const formatDate = (timestamp: number, lang: string): string => {
  return new Date(timestamp * 1000).toLocaleString(lang, {
    month: "short",
    day: "numeric",
    year: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
};

const MeetingItem: React.FC<{
  meeting: MeetingSummary;
  onSelect: () => void;
  onDelete: () => void;
}> = ({ meeting, onSelect, onDelete }) => {
  const { i18n } = useTranslation();

  return (
    <div className="px-4 py-3 flex items-center justify-between hover:bg-mid-gray/10 transition-colors">
      <button
        className="flex-1 flex items-center gap-3 text-left cursor-pointer"
        onClick={onSelect}
      >
        <div className="flex-1 min-w-0">
          <p className="text-sm font-medium truncate">{meeting.title}</p>
          <div className="flex items-center gap-2 text-xs text-text/50">
            <span>{formatDate(meeting.started_at, i18n.language)}</span>
            <span className="flex items-center gap-1">
              <Clock className="w-3 h-3" />
              {formatDuration(meeting.duration_seconds)}
            </span>
            <span className="capitalize">{meeting.status}</span>
          </div>
        </div>
        <ChevronRight className="w-4 h-4 text-text/30 shrink-0" />
      </button>
      <button
        onClick={(e) => {
          e.stopPropagation();
          onDelete();
        }}
        className="ml-2 p-1 text-text/40 hover:text-red-400 transition-colors cursor-pointer"
      >
        <Trash2 className="w-4 h-4" />
      </button>
    </div>
  );
};

export const MeetingHistory: React.FC = () => {
  const { t } = useTranslation();
  const meetings = useMeetingStore((s) => s.meetings);
  const isLoading = useMeetingStore((s) => s.isLoading);
  const deleteMeeting = useMeetingStore((s) => s.deleteMeeting);
  const [selectedMeetingId, setSelectedMeetingId] = useState<string | null>(
    null,
  );

  if (selectedMeetingId) {
    const meeting = meetings.find((m) => m.id === selectedMeetingId);
    return (
      <MeetingTranscriptViewer
        meetingId={selectedMeetingId}
        title={meeting?.title ?? "Meeting"}
        onBack={() => setSelectedMeetingId(null)}
      />
    );
  }

  return (
    <div className="space-y-2">
      <div className="px-4">
        <h2 className="text-xs font-medium text-mid-gray uppercase tracking-wide">
          {t("meeting.history.title")}
        </h2>
      </div>
      <div className="bg-background border border-mid-gray/20 rounded-lg overflow-hidden">
        {isLoading ? (
          <div className="px-4 py-3 text-center text-text/60 text-sm">
            {t("meeting.history.loading")}
          </div>
        ) : meetings.length === 0 ? (
          <div className="px-4 py-3 text-center text-text/60 text-sm">
            {t("meeting.history.empty")}
          </div>
        ) : (
          <div className="divide-y divide-mid-gray/20">
            {meetings.map((meeting) => (
              <MeetingItem
                key={meeting.id}
                meeting={meeting}
                onSelect={() => setSelectedMeetingId(meeting.id)}
                onDelete={() => deleteMeeting(meeting.id)}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
};
