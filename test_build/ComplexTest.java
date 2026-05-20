package com.example.tinyart;

public class ComplexTest {
    public static void testAll() {
        System.out.println("Starting Complex Test...");
        
        // 1. Test Switch
        testSwitch(2);
        testSwitch(10);
        
        // 2. Test Float/Double math
        testMath(10.5f, 2.0f);
        
        // 3. Test Type Checking
        Object obj = "Hello";
        if (obj instanceof String) {
            System.out.println("Instanceof works: String detected");
        }
        
        System.out.println("Complex Test Finished!");
    }

    private static void testSwitch(int val) {
        switch (val) {
            case 1: System.out.println("Switch: One"); break;
            case 2: System.out.println("Switch: Two"); break;
            case 10: System.out.println("Switch: Ten (Sparse)"); break;
            default: System.out.println("Switch: Default"); break;
        }
    }

    private static void testMath(float a, float b) {
        float res = (a * b) / 2.0f;
        System.out.println("Math result: ");
        if (res == 10.5f) {
            System.out.println("Float math OK");
        }
    }
}
