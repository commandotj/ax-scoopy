export type RecorderPhase =
	| "idle"
	| "recording"
	| "paused"
	| "creating"
	| "converting"
	| "uploading"
	| "completed"
	| "error";

export type RecorderErrorEvent = Event & { error?: DOMException };

export type UploadTarget =
	| {
			type: "s3Post";
			url: string;
			fields: Record<string, string>;
	  }
	| {
			type: "put";
			url: string;
			headers: Record<string, string>;
	  }
	| {
			type: "driveResumable";
			url: string;
			headers: Record<string, string>;
	  };

export type UploadStatus =
	| {
			status: "parsing";
	  }
	| {
			status: "creating";
	  }
	| {
			status: "converting";
			scoopyId: string;
			progress: number;
	  }
	| {
			status: "uploadingThumbnail";
			scoopyId: string;
			progress: number;
	  }
	| {
			status: "uploadingVideo";
			scoopyId: string;
			progress: number;
			thumbnailUrl: string | undefined;
	  }
	| {
			status: "serverProcessing";
			scoopyId: string;
	  };

export type VideoId = string & { readonly __brand: "VideoId" };

export type ChunkUploadState = {
	partNumber: number;
	sizeBytes: number;
	uploadedBytes: number;
	progress: number;
	status: "queued" | "uploading" | "complete" | "error";
};

export type RecordingFailureDownload = {
	url: string;
	fileName: string;
};

export type RecoveredRecordingDownload = RecordingFailureDownload & {
	id: string;
	createdAt: number;
};
