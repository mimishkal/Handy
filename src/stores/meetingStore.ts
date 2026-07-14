import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { commands } from "@/bindings";
import type { MeetingSummary, MeetingSegment } from "@/bindings";

interface MeetingSegmentEvent {
  session_id: string;
  segment_index: number;
  text: string;
  speaker: string;
  started_at: number;
}

interface MeetingStateEvent {
  session_id: string;
  status: string;
}

interface MeetingStore {
  // State
  activeSessionId: string | null;
  activeStatus: string | null;
  liveSegments: MeetingSegmentEvent[];
  meetings: MeetingSummary[];
  isLoading: boolean;

  // Actions
  initialize: () => Promise<void>;
  startMeeting: () => Promise<string | null>;
  stopMeeting: () => Promise<void>;
  cancelMeeting: () => Promise<void>;
  loadMeetings: () => Promise<void>;
  loadTranscript: (sessionId: string) => Promise<MeetingSegment[]>;
  deleteMeeting: (sessionId: string) => Promise<void>;
}

let initialized = false;

export const useMeetingStore = create<MeetingStore>((set, get) => ({
  activeSessionId: null,
  activeStatus: null,
  liveSegments: [],
  meetings: [],
  isLoading: false,

  initialize: async () => {
    if (initialized) return;
    initialized = true;

    // Listen for live segments
    listen<MeetingSegmentEvent>("meeting-segment", (event) => {
      const seg = event.payload;
      set((state) => ({
        liveSegments: [...state.liveSegments, seg],
      }));
    });

    // Listen for state changes
    listen<MeetingStateEvent>("meeting-state-changed", (event) => {
      const { session_id, status } = event.payload;
      if (status === "recording") {
        set({
          activeSessionId: session_id,
          activeStatus: status,
          liveSegments: [],
        });
      } else {
        set({
          activeSessionId: null,
          activeStatus: null,
        });
        // Refresh meeting list when a meeting completes/cancels
        get().loadMeetings();
      }
    });

    await get().loadMeetings();
  },

  startMeeting: async () => {
    const result = await commands.startMeeting();
    if (result.status === "ok") {
      set({
        activeSessionId: result.data,
        activeStatus: "recording",
        liveSegments: [],
      });
      return result.data;
    }
    console.error("Failed to start meeting:", result.error);
    return null;
  },

  stopMeeting: async () => {
    const sessionId = get().activeSessionId;
    if (!sessionId) return;
    const result = await commands.stopMeeting(sessionId);
    if (result.status === "error") {
      console.error("Failed to stop meeting:", result.error);
    }
  },

  cancelMeeting: async () => {
    const sessionId = get().activeSessionId;
    if (!sessionId) return;
    const result = await commands.cancelMeeting(sessionId);
    if (result.status === "error") {
      console.error("Failed to cancel meeting:", result.error);
    }
  },

  loadMeetings: async () => {
    set({ isLoading: true });
    const result = await commands.getMeetings();
    if (result.status === "ok") {
      set({ meetings: result.data, isLoading: false });
    } else {
      set({ isLoading: false });
    }
  },

  loadTranscript: async (sessionId: string) => {
    const result = await commands.getMeetingTranscript(sessionId);
    if (result.status === "ok") {
      return result.data;
    }
    return [];
  },

  deleteMeeting: async (sessionId: string) => {
    const result = await commands.deleteMeeting(sessionId);
    if (result.status === "ok") {
      set((state) => ({
        meetings: state.meetings.filter((m) => m.id !== sessionId),
      }));
    }
  },
}));
