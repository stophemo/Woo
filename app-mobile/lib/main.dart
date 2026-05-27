import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'app.dart';

void main() async {
  WidgetsFlutterBinding.ensureInitialized();

  // Database init is handled inside the provider system
  runApp(
    const ProviderScope(
      child: WooApp(),
    ),
  );
}
