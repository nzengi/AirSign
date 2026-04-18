import React, { useEffect, useRef } from "react";
import { Animated, Easing, StyleSheet, View } from "react-native";
import QRCode from "react-native-qrcode-svg";

interface Props {
  /** Array of base64-encoded fountain frames to cycle through */
  frames: string[];
  /** Milliseconds to display each frame (default 200ms → 5 fps) */
  frameIntervalMs?: number;
  size?: number;
}

/**
 * QrAnimator — cycles through fountain-code QR frames at a fixed interval.
 *
 * The receiving device scans these frames until the fountain decoder
 * has enough to reconstruct the original payload.
 */
export default function QrAnimator({
  frames,
  frameIntervalMs = 200,
  size = 280,
}: Props) {
  const indexRef = useRef(0);
  const [currentFrame, setCurrentFrame] = React.useState(frames[0] ?? "");
  const fadeAnim = useRef(new Animated.Value(1)).current;

  useEffect(() => {
    if (frames.length === 0) return;

    const tick = () => {
      // Fade out → swap frame → fade in
      Animated.timing(fadeAnim, {
        toValue: 0.3,
        duration: frameIntervalMs * 0.2,
        easing: Easing.out(Easing.ease),
        useNativeDriver: true,
      }).start(() => {
        indexRef.current = (indexRef.current + 1) % frames.length;
        setCurrentFrame(frames[indexRef.current]);

        Animated.timing(fadeAnim, {
          toValue: 1,
          duration: frameIntervalMs * 0.2,
          easing: Easing.in(Easing.ease),
          useNativeDriver: true,
        }).start();
      });
    };

    const id = setInterval(tick, frameIntervalMs);
    return () => clearInterval(id);
  }, [frames, frameIntervalMs, fadeAnim]);

  if (frames.length === 0) {
    return <View style={[styles.placeholder, { width: size, height: size }]} />;
  }

  return (
    <Animated.View style={[styles.container, { opacity: fadeAnim }]}>
      <View style={styles.qrWrapper}>
        <QRCode
          value={currentFrame}
          size={size}
          backgroundColor="#ffffff"
          color="#000000"
          errorCorrectionLevel="M"
        />
      </View>
    </Animated.View>
  );
}

const styles = StyleSheet.create({
  container: {
    alignItems: "center",
    justifyContent: "center",
  },
  qrWrapper: {
    padding: 12,
    backgroundColor: "#ffffff",
    borderRadius: 8,
  },
  placeholder: {
    backgroundColor: "#1f2937",
    borderRadius: 8,
  },
});