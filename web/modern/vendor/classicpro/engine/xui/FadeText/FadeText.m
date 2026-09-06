#include <lib/std.mi>

Function toggleFade();
Function setMyText(String input);

Global Group XUIGroup;
Global Text text1, text2;
Global boolean fadeDirection;
Global String currentText;

System.onScriptLoaded(){
	XUIGroup = getScriptGroup();
	text1 = XUIGroup.findObject("FadeText.1");
	text2 = XUIGroup.findObject("FadeText.2");
	currentText="";
}

System.onSetXuiParam(String param, String value) {
	if(strlower(param) == "text"){
		setMyText(value);
	}
	else if(strlower(param) == "fontsize"){
		text1.setXmlParam("fontsize", value);
		text2.setXmlParam("fontsize", value);
	}
	else if(strlower(param) == "align"){
		text1.setXmlParam("align", value);
		text2.setXmlParam("align", value);
	}
}

setMyText(String input){
	if(input==currentText) return;

	if(fadeDirection){
		text1.setText(input);
	}
	else{
		text2.setText(input);
	}
	currentText=input;
	toggleFade();
}

toggleFade(){
	if(fadeDirection){
		text1.setAlpha(0);
		text2.setAlpha(255);
		
		text1.setTargetA(255);
		text2.setTargetA(0);

	}
	else{
		text1.setAlpha(255);
		text2.setAlpha(0);

		text1.setTargetA(0);
		text2.setTargetA(255);
	}
	
	text1.setTargetSpeed(0.5);
	text2.setTargetSpeed(0.5);

	text1.gotoTarget();
	text2.gotoTarget();
	
	fadeDirection = !fadeDirection;
}