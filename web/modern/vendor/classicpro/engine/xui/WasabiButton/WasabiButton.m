#include <lib/std.mi>

Function setButtonState(int x);

Global Group frameGroup;
Global Button myButton;
Global GuiObject myGrid;
Global boolean isMouseButtonDown;

System.onScriptLoaded() {
  frameGroup = getScriptGroup();

  myButton = frameGroup.findObject("wasabi.button");
  myGrid = frameGroup.findObject("wasabi.button.grid");
  isMouseButtonDown = false;
}

System.onSetXuiParam(String param, String value){
	if(strlower(param)=="mytooltip"){
		myButton.setXmlParam("tooltip", value);
	}
}

myButton.onEnterArea(){
	if(isMouseButtonDown){
		setButtonState(2);
	}
	else{
		setButtonState(1);
	}
}

myButton.onLeaveArea(){
	setButtonState(0);
}

myButton.onLeftButtonDown(int x, int y){
	setButtonState(2);
  isMouseButtonDown = true;
}
myButton.onLeftButtonUp(int x, int y){
	setButtonState(0);
  isMouseButtonDown = false;
}

myButton.onRightButtonDown(int x, int y){
	setButtonState(2);
  isMouseButtonDown = true;
}
myButton.onRightButtonUp(int x, int y){
	setButtonState(0);
  isMouseButtonDown = false;
}


setButtonState(int x){
	if(x==0){
		myGrid.setXmlParam("topleft", "wasabi.button.top.left");
		myGrid.setXmlParam("top", "wasabi.button.top");
		myGrid.setXmlParam("topright", "wasabi.button.top.right");
		myGrid.setXmlParam("left", "wasabi.button.left");
		myGrid.setXmlParam("middle", "wasabi.button.middle");
		myGrid.setXmlParam("right", "wasabi.button.right");
		myGrid.setXmlParam("bottomleft", "wasabi.button.bottom.left");
		myGrid.setXmlParam("bottom", "wasabi.button.bottom");
		myGrid.setXmlParam("bottomright", "wasabi.button.bottom.right");
	}
	else if(x==1){
		myGrid.setXmlParam("topleft", "wasabi.button.hover.top.left");
		myGrid.setXmlParam("top", "wasabi.button.hover.top");
		myGrid.setXmlParam("topright", "wasabi.button.hover.top.right");
		myGrid.setXmlParam("left", "wasabi.button.hover.left");
		myGrid.setXmlParam("middle", "wasabi.button.hover.middle");
		myGrid.setXmlParam("right", "wasabi.button.hover.right");
		myGrid.setXmlParam("bottomleft", "wasabi.button.hover.bottom.left");
		myGrid.setXmlParam("bottom", "wasabi.button.hover.bottom");
		myGrid.setXmlParam("bottomright", "wasabi.button.hover.bottom.right");
	}
	else if(x==2){
		myGrid.setXmlParam("topleft", "wasabi.button.pressed.top.left");
		myGrid.setXmlParam("top", "wasabi.button.pressed.top");
		myGrid.setXmlParam("topright", "wasabi.button.pressed.top.right");
		myGrid.setXmlParam("left", "wasabi.button.pressed.left");
		myGrid.setXmlParam("middle", "wasabi.button.pressed.middle");
		myGrid.setXmlParam("right", "wasabi.button.pressed.right");
		myGrid.setXmlParam("bottomleft", "wasabi.button.pressed.bottom.left");
		myGrid.setXmlParam("bottom", "wasabi.button.pressed.bottom");
		myGrid.setXmlParam("bottomright", "wasabi.button.pressed.bottom.right");
	}
}